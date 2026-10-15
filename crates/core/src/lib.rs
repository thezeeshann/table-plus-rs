//! PostgreSQL engine for table-plus-rs. No UI code lives here, so everything is testable
//! without a window: sessions, catalog queries, SQL generation, local storage.

pub mod error;
pub mod export;
pub mod model;
pub mod ops;
pub mod pg;
pub mod storage;

pub use error::{AppError, AppResult};
pub use pg::session::Session;
