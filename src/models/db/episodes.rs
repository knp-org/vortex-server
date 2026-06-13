//! `episodes` detail row (1:1 with a `media_items` spine row of type `episode`).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Episode {
    pub item_id: i64,
    pub season_id: Option<i64>,
    pub episode_number: Option<i64>,
    pub title: Option<String>,
    pub plot: Option<String>,
    pub still_url: Option<String>,
    pub runtime: Option<i64>,
    pub air_date: Option<String>,
}
