use std::{
    collections::HashMap,
    sync::Arc,
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
    response::IntoResponse,
    Router,
    routing::*,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    let prices = Arc::new(RwLock::new(HashMap::default()));
    let app = app(prices);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3001")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

fn app(state: TPriceMap) -> Router {
    Router::new()
        .route("/prices", get(get_prices).post(create_price))
        .route("/prices/:id", get(get_price_by_id).patch(update_price_by_id).delete(delete_price))
        .with_state(state)
}

async fn get_prices(
    State(prices): State<TPriceMap>,
) -> Result<impl IntoResponse, StatusCode> {
    let prices = prices.read().await;
    Ok(Json(prices.values().cloned().collect::<Vec<TPrice>>()))
}

async fn create_price(
    State(prices): State<TPriceMap>,
    Json(input): Json<PriceDto>,
) -> Result<impl IntoResponse, StatusCode> {
    let uuid = Uuid::new_v4();
    prices.write().await.insert(uuid, input.price);

    Ok(uuid.to_string())
}

async fn get_price_by_id(
    Path(id): Path<Uuid>,
    State(prices): State<TPriceMap>,
) -> Result<impl IntoResponse, StatusCode> {
    match prices.read().await.get(&id) {
        Some(price) => Ok(price.to_string()),
        None => Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PriceDto {
    price: TPrice,
}

async fn update_price_by_id(
    Path(id): Path<Uuid>,
    State(prices): State<TPriceMap>,
    Json(input): Json<PriceDto>,
) -> Result<impl IntoResponse, StatusCode> {
    match prices.write().await.get_mut(&id) {
        Some(old_price) => {
            *old_price = input.price;
            Ok(StatusCode::OK)
        },
        None => Err(StatusCode::NOT_FOUND)
    }
}

async fn delete_price(
    Path(id): Path<Uuid>,
    State(prices): State<TPriceMap>,
) -> Result<impl IntoResponse, StatusCode> {
    match prices.write().await.remove_entry(&id) {
        Some(_) => Ok(StatusCode::OK),
        None => Err(StatusCode::NOT_FOUND)
    }
}

type TPrice = u64;
type TPriceMap = Arc<RwLock<HashMap<Uuid, TPrice>>>;

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{self, Request, StatusCode},
    };
    use axum::body::Bytes;
    use axum::response::Response;
    use axum::routing::RouterIntoService;
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::{Service, ServiceExt};

    use super::*;

    #[tokio::test]
    async fn get_prices_test() {
        let uuid = Uuid::new_v4();
        let map_with_entry = build_test_hashmap_with_entry(uuid, 355);
        
        let state = Arc::new(RwLock::new(map_with_entry));
        let mut app = app(state).into_service();

        let request = build_request(
            http::Method::GET,
            "/prices",
            None
        );

        let response = call(request, &mut app).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(collect_body(response).await, "[355]");
    }
    
    #[tokio::test]
    async fn get_price_by_id_test() {
        let uuid = Uuid::new_v4();
        let map_with_entry = build_test_hashmap_with_entry(uuid, 355);
        let state = Arc::new(RwLock::new(map_with_entry));
        let mut app = app(state).into_service();

        let request = build_request(
            http::Method::GET,
            &format!("/prices/{}", uuid),
            None
        );

        let response = call(request, &mut app).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(collect_body(response).await, "355");
    }

    #[tokio::test]
    async fn get_not_found_price_by_id_test() {
        let state = Arc::new(RwLock::new(HashMap::new()));
        let mut app = app(state).into_service();

        let request = build_request(
            http::Method::GET,
            &format!("/prices/{}", Uuid::new_v4()),
            None
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(collect_body(response).await, "");
    }

    #[tokio::test]
    async fn patch_price_by_id_test() {
        let uuid = Uuid::new_v4();
        let map_with_entry = build_test_hashmap_with_entry(uuid, 355);
        let state = Arc::new(RwLock::new(map_with_entry));
        let mut app = app(state).into_service();

        let request = build_request(
            http::Method::PATCH,
            &format!("/prices/{}", uuid),
            Some(&json!({"price": 235}))
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::OK);

        let request = build_request(
            http::Method::GET,
            &format!("/prices/{}", uuid),
            None
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(collect_body(response).await, "235");
    }

    #[tokio::test]
    async fn delete_price_test() {
        let uuid = Uuid::new_v4();
        let map_with_entry = build_test_hashmap_with_entry(uuid, 355);
        let state = Arc::new(RwLock::new(map_with_entry));
        let mut app = app(state).into_service();

        let request = build_request(
            http::Method::DELETE,
            &format!("/prices/{}", uuid),
            None
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::OK);

        let request = build_request(
            http::Method::GET,
            &format!("/prices/{}", uuid),
            None
        );
        let response = call(request, &mut app).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(collect_body(response).await, "");
    }

    fn build_test_hashmap_with_entry(uuid: Uuid, value: TPrice) -> HashMap<Uuid, TPrice> {
        let mut hashmap_with_test_price_entry: HashMap<Uuid, TPrice> = HashMap::new();
        hashmap_with_test_price_entry.insert(uuid, value);

        hashmap_with_test_price_entry
    }

    fn build_request(method: http::Method, uri: &str, maybe_json: Option<&Value>) -> Request<Body> {
        let body = match maybe_json {
            Some(json) => Body::from(
                serde_json::to_vec(json).unwrap(),
            ),
            None => Body::empty(),
        };

        Request::builder()
            .method(method)
            .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .uri(uri)
            .body(body)
            .unwrap()
    }

    async fn call(request: Request<Body>, app: &mut RouterIntoService<Body>) -> Response<Body> {
        ServiceExt::<Request<Body>>::ready(app)
            .await
            .unwrap()
            .call(request)
            .await
            .unwrap()
    }

    async fn collect_body(response: Response<Body>) -> Bytes {
        response.into_body().collect().await.unwrap().to_bytes()
    }
}
