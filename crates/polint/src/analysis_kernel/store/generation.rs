//! Opaque generation reservation and publication for the semantic store.

use std::num::NonZeroI64;

use rusqlite::{OptionalExtension, Transaction};

use super::connection::{self, ReadOnlyConnection, WriterConnection};
use super::{StoreStatus, map_connection_error};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GenerationHandle(NonZeroI64);

impl GenerationHandle {
    fn from_scalar(value: i64) -> Result<Self, GenerationError> {
        NonZeroI64::new(value)
            .filter(|value| value.get().is_positive())
            .map(Self)
            .ok_or(GenerationError::InvalidSelection)
    }

    fn scalar(self) -> i64 {
        self.0.get()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GenerationError {
    Store(StoreStatus),
    InvalidTransition,
    InvalidSelection,
}

impl From<connection::ConnectionError> for GenerationError {
    fn from(error: connection::ConnectionError) -> Self {
        Self::Store(map_connection_error(error))
    }
}

pub(super) fn reserve(writer: &mut WriterConnection) -> Result<GenerationHandle, GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        let scalar = transaction
            .query_row(
                "INSERT INTO generations (status) VALUES ('pending') \
                 RETURNING generation_id",
                [],
                |row| row.get(0),
            )
            .map_err(connection::classify_sqlite_error)?;
        GenerationHandle::from_scalar(scalar)
    })
}

pub(super) fn publish(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
) -> Result<GenerationHandle, GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        let changed = transaction
            .execute(
                "UPDATE generations SET status = 'complete' \
                 WHERE generation_id = ?1 AND status = 'pending'",
                [handle.scalar()],
            )
            .map_err(connection::classify_sqlite_error)?;
        if changed != 1 {
            return Err(GenerationError::InvalidTransition);
        }

        select_complete(transaction, handle)?;
        Ok(handle)
    })
}

pub(super) fn active(
    reader: &ReadOnlyConnection,
) -> Result<Option<GenerationHandle>, GenerationError> {
    connection::with_read_connection(reader, |connection| {
        let selected = connection
            .query_row(
                "SELECT active.singleton, active.generation_id, active.required_status, \
                        generation.generation_id, generation.status \
                 FROM active_generation AS active \
                 LEFT JOIN generations AS generation \
                   ON generation.generation_id = active.generation_id \
                  AND generation.status = active.required_status",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(connection::classify_sqlite_error)?;

        let Some((singleton, scalar, required_status, joined_scalar, generation_status)) = selected
        else {
            return Ok(None);
        };
        if singleton != 1
            || required_status != "complete"
            || joined_scalar != Some(scalar)
            || generation_status.as_deref() != Some("complete")
        {
            return Err(GenerationError::InvalidSelection);
        }
        GenerationHandle::from_scalar(scalar).map(Some)
    })
}

fn select_complete(
    transaction: &Transaction<'_>,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    let is_complete = transaction
        .query_row(
            "SELECT status = 'complete' FROM generations WHERE generation_id = ?1",
            [handle.scalar()],
            |row| row.get::<_, bool>(0),
        )
        .optional()
        .map_err(connection::classify_sqlite_error)?;
    if is_complete != Some(true) {
        return Err(GenerationError::InvalidTransition);
    }

    transaction
        .execute(
            "INSERT INTO active_generation (singleton, generation_id, required_status) \
             VALUES (1, ?1, 'complete') \
             ON CONFLICT(singleton) DO UPDATE SET \
                 generation_id = excluded.generation_id, \
                 required_status = excluded.required_status",
            [handle.scalar()],
        )
        .map_err(connection::classify_sqlite_error)?;

    let authentic: i64 = transaction
        .query_row(
            "SELECT count(*) \
             FROM active_generation AS active \
             JOIN generations AS generation \
               ON generation.generation_id = active.generation_id \
              AND generation.status = active.required_status \
             WHERE active.singleton = 1 \
               AND active.generation_id = ?1 \
               AND active.required_status = 'complete' \
               AND generation.status = 'complete'",
            [handle.scalar()],
            |row| row.get(0),
        )
        .map_err(connection::classify_sqlite_error)?;
    if authentic != 1 {
        return Err(GenerationError::InvalidSelection);
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn select_for_test(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        select_complete(transaction, handle)
    })
}

#[cfg(test)]
pub(super) fn select_without_validation_for_test(
    writer: &mut WriterConnection,
    handle: GenerationHandle,
) -> Result<(), GenerationError> {
    connection::with_immediate_transaction(writer, |transaction| {
        transaction
            .execute(
                "INSERT INTO active_generation (singleton, generation_id, required_status) \
                 VALUES (1, ?1, 'complete') \
                 ON CONFLICT(singleton) DO UPDATE SET \
                     generation_id = excluded.generation_id, \
                     required_status = excluded.required_status",
                [handle.scalar()],
            )
            .map_err(connection::classify_sqlite_error)?;
        Ok(())
    })
}
