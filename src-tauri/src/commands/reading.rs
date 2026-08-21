//! Reading recap commands (MISSION-083). Thin handler — the recap assembly
//! lives in `application::reading_recap_service`.

use sqlx::SqlitePool;
use tauri::command;
use tauri::State;
use tracing::info;

use crate::application::reading_recap_service::{ReadingRecap, ReadingRecapService};
use crate::error::AppError;

/// Resolve the reading recap for one year: pages & chapters consumed per
/// month (bucketed by local time), the year totals, and the all-time mood /
/// pace / format taste distributions.
#[command]
pub async fn reading_recap(
    state: State<'_, SqlitePool>,
    year: u16,
) -> Result<ReadingRecap, AppError> {
    info!("reading_recap invoked");
    let service = ReadingRecapService::new(state.inner().clone());
    service.recap(year).await
}
