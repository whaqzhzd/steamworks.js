use napi_derive::napi;

#[napi]
pub mod stats {
    use napi::bindgen_prelude::{BigInt, Error, ToNapiValue};
    use steamworks::{
        Leaderboard, LeaderboardDataRequest, LeaderboardDisplayType, LeaderboardSortMethod,
        UploadScoreMethod,
    };
    use tokio::sync::oneshot;

    #[napi]
    pub struct LeaderboardInfo {
        pub user: BigInt,
        pub global_rank: i32,
        pub score: i32,
        pub details: Vec<i32>,
    }

    #[napi]
    pub struct LeaderboardUploadedScore {
        pub score: i32,
        pub was_changed: bool,
        pub global_rank_new: i32,
        pub global_rank_previous: i32,
    }

    #[napi]
    pub fn get_int(name: String) -> Option<i32> {
        let client = crate::client::get_client();
        let result = client.user_stats().get_stat_i32(&name);

        match result {
            Ok(stat) => Some(stat),
            Err(()) => None,
        }
    }

    #[napi]
    pub fn set_int(name: String, value: i32) -> bool {
        let client = crate::client::get_client();
        let result = client.user_stats().set_stat_i32(&name, value);
        result.is_ok()
    }

    #[napi]
    pub fn store() -> bool {
        let client = crate::client::get_client();
        let result = client.user_stats().store_stats();
        result.is_ok()
    }

    #[napi]
    pub fn reset_all(achievements_too: bool) -> bool {
        let client = crate::client::get_client();
        let result = client.user_stats().reset_all_stats(achievements_too);
        result.is_ok()
    }

    #[napi]
    pub fn get_leaderboard_entry_count(id: BigInt) -> i32 {
        let client = crate::client::get_client();
        client
            .user_stats()
            .get_leaderboard_entry_count(&Leaderboard::new(id.get_u64().1))
    }

    #[napi]
    pub async fn upload_leaderboard_score(
        id: BigInt,
        method: u32,
        score: i32,
        details: Vec<i32>,
    ) -> Result<Option<LeaderboardUploadedScore>, Error> {
        let client = crate::client::get_client();

        let (tx, rx) = oneshot::channel();

        let m = if method == 0 {
            UploadScoreMethod::KeepBest
        } else {
            UploadScoreMethod::ForceUpdate
        };

        client.user_stats().upload_leaderboard_score(
            &Leaderboard::new(id.get_u64().1),
            m,
            score,
            &details,
            |result| {
                tx.send(result).unwrap();
            },
        );

        let result = rx.await.unwrap();
        match result {
            Ok(leader_board) => {
                if leader_board.is_none() {
                    return Ok(None);
                }

                return Ok(Some(LeaderboardUploadedScore {
                    score: leader_board.as_ref().unwrap().score,
                    was_changed: leader_board.as_ref().unwrap().was_changed,
                    global_rank_new: leader_board.as_ref().unwrap().global_rank_new,
                    global_rank_previous: leader_board.as_ref().unwrap().global_rank_previous,
                }));
            }
            Err(e) => Err(Error::from_reason(e.to_string())),
        }
    }

    #[napi]
    pub async fn download_leaderboard_entries(
        id: BigInt,
        start: u32,
        end: u32,
        max_details_len: u32,
    ) -> Result<Vec<LeaderboardInfo>, Error> {
        let client = crate::client::get_client();

        let (tx, rx) = oneshot::channel();
        client.user_stats().download_leaderboard_entries(
            &Leaderboard::new(id.get_u64().1),
            LeaderboardDataRequest::Global,
            start as usize,
            end as usize,
            max_details_len as usize,
            |result| {
                tx.send(result).unwrap();
            },
        );

        let result = rx.await.unwrap();
        match result {
            Ok(leader_board) => Ok(leader_board
                .iter()
                .map(|l| LeaderboardInfo {
                    user: BigInt::from(l.user.raw()),
                    global_rank: l.global_rank,
                    score: l.score,
                    details: l.details.clone(),
                })
                .collect::<Vec<LeaderboardInfo>>()),
            Err(e) => Err(Error::from_reason(e.to_string())),
        }
    }

    #[napi]
    pub async fn find_or_create_leaderboard(name: String) -> Result<BigInt, Error> {
        let client = crate::client::get_client();
        let (tx, rx) = oneshot::channel();

        client.user_stats().find_or_create_leaderboard(
            name.as_str(),
            LeaderboardSortMethod::Ascending,
            LeaderboardDisplayType::TimeMilliSeconds,
            |result| {
                tx.send(result).unwrap();
            },
        );

        let result = rx.await.unwrap();
        match result {
            Ok(leader_board) => {
                if leader_board.is_none() {
                    return Ok(BigInt::from(0u64));
                };

                return Ok(BigInt::from(leader_board.unwrap().raw()));
            }
            Err(e) => Err(Error::from_reason(e.to_string())),
        }
    }
}
