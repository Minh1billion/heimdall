pub mod approvals;
pub mod chat;
pub mod meta_connections;
pub mod reports;
pub mod routes;
pub mod settings;

use crate::container::Container;
use crate::router::Router;
use std::sync::Arc;

pub fn build_router(container: Arc<Container>) -> Router {
    let mut router = Router::new(container);
    routes::register(&mut router);
    settings::register(&mut router);
    meta_connections::register(&mut router);
    chat::register(&mut router);
    approvals::register(&mut router);
    reports::register(&mut router);
    router
}
