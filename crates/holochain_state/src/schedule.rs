use crate::prelude::StateMutationResult;
use holochain_sqlite::rusqlite::OptionalExtension;
use holochain_sqlite::rusqlite::{named_params, Transaction};
use holochain_zome_types::ScheduledFn;
use holochain_zome_types::Timestamp;
use holochain_zome_types::ZomeName;

pub fn fn_is_scheduled(txn: &Transaction, scheduled_fn: ScheduledFn) -> StateMutationResult<bool> {
    match txn
        .query_row(
            "
            SELECT 1
            FROM ScheduledFunctions
            WHERE
            zome_name = :zome_name
            AND scheduled_fn = :scheduled_fn
            LIMIT 1
            ",
            named_params! {
                ":zome_name": scheduled_fn.zome_name().to_string(),
                ":scheduled_fn": scheduled_fn.fn_name(),
            },
            |row| row.get::<_, u32>(0),
        )
        .optional()?
    {
        Some(_) => Ok(true),
        None => Ok(false),
    }
}

pub fn live_scheduled_fns(
    txn: &Transaction,
    now: Timestamp,
) -> StateMutationResult<Vec<(ScheduledFn, Schedule)>> {
    let mut stmt = txn.prepare(
        "
        SELECT
        zome_name,
        scheduled_fn,
        schedule,
        ephemeral
        FROM ScheduledFunctions
        WHERE
        start <= ?
        AND ? <= end",
    )?;
    let rows = stmt.query_map([now], |row| {
        Ok((
            ScheduledFn::new(ZomeName(row.get(0)?), row.get(1)?),
            if row.get(3)? {
                Schedule::Ephemeral
            }
            row.get(2)?,
        ))
    })?;
    let mut ret = vec![];
    for row in rows {
        ret.push(row?)
    }
    Ok(ret)
}
