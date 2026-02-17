use std::sync::Arc;

use crate::db::{Clip, Database};

pub fn run_prune(
    db: &Arc<Database>,
    history_limit: i64,
) -> Result<Vec<Clip>, crate::error::AppError> {
    db.prune_excess(history_limit)
        .map_err(crate::error::AppError::from)
}
