use std::collections::HashMap;

use chrono::Duration;
use deltalake::{operations::vacuum::VacuumBuilder, DeltaTableBuilder};
use libc::c_void;

use crate::{
    error::{DeltaTableError, DeltaTableErrorCode},
    runtime::Runtime,
    ByteArray, ByteArrayRef, DynamicArray, Map, SerializedBuffer,
};

pub struct RawDeltaTable {
    table: deltalake::DeltaTable,
}

#[repr(C)]
pub struct TableOptions {
    version: i64,
    storage_options: *const Map,
    without_files: bool,
    log_buffer_size: libc::size_t,
}

#[repr(C)]
pub struct TableOrFail {
    runtime: *mut RawDeltaTable,
    fail: *mut DeltaTableError,
}

#[repr(C)]
pub struct BytesOrError {
    bytes: *const ByteArray,
    error: *const DeltaTableError,
}

#[repr(C)]
pub struct GenericOrError {
    bytes: *const c_void,
    error: *const DeltaTableError,
}

#[repr(C)]
pub struct VacuumOptions {
    dry_run: bool,
    retention_hours: u64,
    enforce_retention_duration: bool,
    custom_metadata: *const Map,
}

type TableNewCallback =
    unsafe extern "C" fn(success: *mut RawDeltaTable, fail: *const DeltaTableError);

type TableEmptyCallback = unsafe extern "C" fn(fail: *const DeltaTableError);

type GenericErrorCallback =
    unsafe extern "C" fn(success: *const c_void, fail: *const DeltaTableError);

#[no_mangle]
pub extern "C" fn table_uri(table: *const RawDeltaTable) -> *mut ByteArray {
    let table = unsafe { &*table };
    let uri = table.table.table_uri();
    ByteArray::from_utf8(uri).into_raw()
}

#[no_mangle]
pub extern "C" fn table_free(table: *mut RawDeltaTable) {
    unsafe {
        let _ = Box::from_raw(table);
    }
}

#[no_mangle]
pub extern "C" fn table_new(
    runtime: *mut Runtime,
    table_uri: *const ByteArrayRef,
    table_options: *const TableOptions,
    callback: TableNewCallback,
) {
    let (runtime, options) = unsafe { (&mut *runtime, &*table_options) };
    let table_uri = unsafe {
        let uri = &*table_uri;
        match std::str::from_utf8(uri.to_slice()) {
            Ok(table_uri) => table_uri,
            Err(err) => {
                callback(
                    std::ptr::null_mut(),
                    Box::into_raw(Box::new(DeltaTableError::new(
                        runtime,
                        DeltaTableErrorCode::Utf8,
                        &err.to_string(),
                    ))),
                );
                return;
            }
        }
    };

    let mut builder = DeltaTableBuilder::from_uri(table_uri);

    if options.version > 0 {
        builder = builder.with_version(options.version)
    }

    if !options.storage_options.is_null() {
        let storage_options = unsafe { &*options.storage_options };
        builder = builder.with_storage_options(storage_options.data.clone());
    }

    if options.without_files {
        builder = builder.without_files();
    }

    if options.log_buffer_size > 0 {
        builder = builder
            .with_log_buffer_size(options.log_buffer_size)
            // unwrap is safe because it only errors when the size is negative
            .unwrap();
    }

    let runtime_handle = runtime.handle();
    std::println!("spawning !");
    runtime_handle.spawn(async move {
        // panic!("this isn't working");
        std::println!("spawned!");
        match builder.load().await {
            Ok(table) => unsafe {
                std::println!("returned ok");
                callback(
                    Box::into_raw(Box::new(RawDeltaTable::new(table))),
                    std::ptr::null(),
                )
            },
            Err(err) => unsafe {
                std::println!("returned error");
                callback(
                    std::ptr::null_mut(),
                    Box::into_raw(Box::new(DeltaTableError::from_error(runtime, err))),
                )
            },
        }
    });
}

#[no_mangle]
pub extern "C" fn table_file_uris(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
) -> GenericOrError {
    do_with_table_and_runtime_sync(runtime, table, |rt, tbl| match tbl.table.get_file_uris() {
        Ok(file_uris) => unsafe {
            GenericOrError {
                bytes: Box::into_raw(Box::new(DynamicArray::from_vec_string(file_uris.collect())))
                    as *const c_void,
                error: std::ptr::null(),
            }
        },
        Err(err) => unsafe {
            GenericOrError {
                bytes: std::ptr::null(),
                error: DeltaTableError::from_error(rt, err).into_raw(),
            }
        },
    })
}

#[no_mangle]
pub extern "C" fn table_files(runtime: *mut Runtime, table: *mut RawDeltaTable) -> GenericOrError {
    do_with_table_and_runtime_sync(runtime, table, |rt, tbl| match tbl.table.get_files_iter() {
        Ok(paths) => unsafe {
            GenericOrError {
                bytes: Box::into_raw(Box::new(DynamicArray::from_vec_string(
                    paths.map(|p| p.to_string()).collect(),
                ))) as *const c_void,
                error: std::ptr::null(),
            }
        },
        Err(err) => unsafe {
            GenericOrError {
                bytes: std::ptr::null(),
                error: DeltaTableError::from_error(rt, err).into_raw(),
            }
        },
    })
}

#[no_mangle]
pub extern "C" fn history(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    limit: usize,
    callback: GenericErrorCallback,
) {
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn table_update_incremental(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    callback: TableEmptyCallback,
) {
    do_with_table_and_runtime(runtime, table, move |rt, tbl| async move {
        match tbl.table.update_incremental(None).await {
            Ok(_) => unsafe {
                callback(std::ptr::null());
            },
            Err(err) => unsafe {
                let error = DeltaTableError::from_error(rt, err);
                callback(Box::into_raw(Box::new(error)))
            },
        };
    });
}

#[no_mangle]
pub extern "C" fn table_load_version(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    version: i64,
    callback: TableEmptyCallback,
) {
    do_with_table_and_runtime(runtime, table, move |rt, tbl| async move {
        match tbl.table.load_version(version).await {
            Ok(_) => unsafe { callback(std::ptr::null()) },
            Err(err) => {
                let error = DeltaTableError::from_error(rt, err);
                unsafe { callback(Box::into_raw(Box::new(error))) }
            }
        };
    })
}

#[no_mangle]
pub extern "C" fn table_load_with_datetime(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    ts_milliseconds: i64,
    callback: TableEmptyCallback,
) {
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn table_merge(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    version: i64,
    callback: TableEmptyCallback,
) {
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn table_protocol(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    version: i64,
    callback: TableEmptyCallback,
) {
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn table_restore(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    version: i64,
    callback: TableEmptyCallback,
) {
    unimplemented!()
}

#[no_mangle]
pub extern "C" fn table_update(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    version: i64,
    callback: TableEmptyCallback,
) {
    unimplemented!()
}

/// Must free the error, but there is no need to free the SerializedBuffer
#[no_mangle]
pub extern "C" fn table_schema(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    callback: GenericErrorCallback,
) {
    do_with_table_and_runtime_sync(
        runtime,
        table,
        move |rt, tbl| match crate::schema::get_schema(rt, &tbl.table) {
            Ok(schema) => {
                let (array, offset) = crate::schema::serialize_schema(rt, &schema);
                let fb = SerializedBuffer {
                    data: array.as_ptr(),
                    size: array.len() - offset,
                    offset,
                };
                unsafe {
                    callback(std::ptr::addr_of!(fb) as *const c_void, std::ptr::null());
                }
            }
            Err(err) => unsafe {
                callback(std::ptr::null(), err.into_raw());
            },
        },
    )
}

#[no_mangle]
pub extern "C" fn table_checkpoint(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    callback: TableEmptyCallback,
) {
    do_with_table_and_runtime(runtime, table, move |rt, tbl| async move {
        match deltalake::checkpoints::create_checkpoint(&tbl.table).await {
            Ok(_) => unsafe {
                callback(std::ptr::null());
            },
            Err(err) => {
                let error =
                    DeltaTableError::new(rt, DeltaTableErrorCode::Protocol, &err.to_string());
                unsafe { callback(error.into_raw()) }
            }
        };
    })
}

#[no_mangle]
pub extern "C" fn table_vacuum(
    runtime: *mut Runtime,
    table: *mut RawDeltaTable,
    options: *const VacuumOptions,
    callback: GenericErrorCallback,
) {
    let (dry_run, retention_hours, enforce_retention_duration, custom_metadata) = unsafe {
        let options = &*options;
        let retention_hours = if options.retention_hours > 0 {
            Some(options.retention_hours)
        } else {
            None
        };
        let custom_metadata = if options.custom_metadata.is_null() {
            None
        } else {
            let map = &*options.custom_metadata;
            Some(map.data.clone())
        };
        (
            options.dry_run,
            retention_hours,
            options.enforce_retention_duration,
            custom_metadata,
        )
    };
    do_with_table_and_runtime(runtime, table, move |rt, tbl| async move {
        match vacuum(
            &mut tbl.table,
            dry_run,
            retention_hours,
            enforce_retention_duration,
            custom_metadata,
        )
        .await
        {
            Ok(strings) => {
                let dyn_array = Box::into_raw(Box::new(DynamicArray::from_vec_string(strings)));
                unsafe {
                    callback(dyn_array as *const c_void, std::ptr::null());
                }
            }
            Err(err) => {
                let error = DeltaTableError::from_error(rt, err);
                unsafe { callback(std::ptr::null_mut(), Box::into_raw(Box::new(error))) }
            }
        };
    });
}

async fn vacuum(
    table: &mut deltalake::DeltaTable,
    dry_run: bool,
    retention_hours: Option<u64>,
    enforce_retention_duration: bool,
    custom_metadata: Option<HashMap<String, String>>,
) -> Result<Vec<String>, deltalake::DeltaTableError> {
    if table.state.is_none() {
        return Err(deltalake::DeltaTableError::NoMetadata);
    }

    let mut cmd = VacuumBuilder::new(table.log_store(), table.state.clone().unwrap())
        .with_enforce_retention_duration(enforce_retention_duration)
        .with_dry_run(dry_run);

    if let Some(retention_period) = retention_hours {
        cmd = cmd.with_retention_period(Duration::hours(retention_period as i64));
    }

    if let Some(metadata) = custom_metadata {
        let json_metadata: serde_json::Map<String, serde_json::Value> =
            metadata.into_iter().map(|(k, v)| (k, v.into())).collect();
        cmd = cmd.with_metadata(json_metadata);
    };

    let (result, metrics) = cmd.await?;
    table.state = result.state;
    Ok(metrics.files_deleted)
}

#[no_mangle]
pub extern "C" fn table_version(table_handle: *mut RawDeltaTable) -> i64 {
    do_with_table(table_handle, |table| table.table.version())
}

#[no_mangle]
pub extern "C" fn table_metadata(table_handle: *mut RawDeltaTable, callback: TableEmptyCallback) {
    do_with_table(table_handle, |table| match table.table.metadata() {
        Ok(_) => todo!(),
        Err(_) => todo!(),
    })
}

fn do_with_table_and_runtime<'a, F, Fut>(rt: *mut Runtime, table: *mut RawDeltaTable, work: F)
where
    F: FnOnce(&'a mut Runtime, &'a mut RawDeltaTable) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    let runtime = unsafe { &mut *rt };
    let table = unsafe { &mut *table };
    let runtime_handle = runtime.handle();
    runtime_handle.spawn(async move {
        work(runtime, table).await;
    });
}

fn do_with_table_and_runtime_sync<'a, F, T>(
    rt: *mut Runtime,
    table: *mut RawDeltaTable,
    work: F,
) -> T
where
    F: FnOnce(&'a mut Runtime, &'a mut RawDeltaTable) -> T,
{
    let runtime = unsafe { &mut *rt };
    let table = unsafe { &mut *table };
    work(runtime, table)
}

fn do_with_table<'a, F, T>(table: *mut RawDeltaTable, work: F) -> T
where
    F: FnOnce(&'a mut RawDeltaTable) -> T,
{
    let table = unsafe { &mut *table };
    work(table)
}

impl RawDeltaTable {
    fn new(table: deltalake::DeltaTable) -> Self {
        RawDeltaTable { table }
    }
}