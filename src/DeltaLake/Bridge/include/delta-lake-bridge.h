#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum DeltaTableErrorCode {
  Utf8 = 0,
  Protocol = 1,
  ObjectStore = 2,
  Parquet = 3,
  Arrow = 4,
  InvalidJsonLog = 5,
  InvalidStatsJson = 6,
  InvalidInvariantJson = 7,
  InvalidVersion = 8,
  MissingDataFile = 9,
  InvalidDateTimeString = 10,
  InvalidData = 11,
  NotATable = 12,
  NoMetadata = 13,
  NoSchema = 14,
  LoadPartitions = 15,
  SchemaMismatch = 16,
  PartitionError = 17,
  InvalidPartitionFilter = 18,
  ColumnsNotPartitioned = 19,
  Io = 20,
  Transaction = 21,
  VersionAlreadyExists = 22,
  VersionMismatch = 23,
  MissingFeature = 24,
  InvalidTableLocation = 25,
  SerializeLogJson = 26,
  SerializeSchemaJson = 27,
  Generic = 28,
  GenericError = 29,
  Kernel = 30,
  MetaDataError = 31,
  NotInitialized = 32,
} DeltaTableErrorCode;

typedef struct CancellationToken CancellationToken;

typedef struct Map Map;

typedef struct RawDeltaTable RawDeltaTable;

typedef struct Runtime Runtime;

typedef struct ByteArrayRef {
  const uint8_t *data;
  size_t size;
} ByteArrayRef;

typedef struct ByteArray {
  const uint8_t *data;
  size_t size;
  /**
   * For internal use only.
   */
  size_t cap;
  /**
   * For internal use only.
   */
  bool disable_free;
} ByteArray;

typedef struct DeltaTableError {
  enum DeltaTableErrorCode code;
  struct ByteArray error;
} DeltaTableError;

/**
 * If fail is not null, it must be manually freed when done. Runtime is always
 * present, but it should never be used if fail is present, only freed after
 * fail is freed using it.
 */
typedef struct RuntimeOrFail {
  struct Runtime *runtime;
  const struct ByteArray *fail;
} RuntimeOrFail;

typedef struct RuntimeOptions {

} RuntimeOptions;

typedef struct DynamicArray {
  const struct ByteArray *data;
  size_t size;
  /**
   * For internal use only.
   */
  size_t cap;
  /**
   * For internal use only.
   */
  bool disable_free;
} DynamicArray;

typedef struct TableOptions {
  int64_t version;
  const struct Map *storage_options;
  bool without_files;
  size_t log_buffer_size;
} TableOptions;

typedef void (*TableNewCallback)(struct RawDeltaTable *success, const struct DeltaTableError *fail);

typedef struct GenericOrError {
  const void *bytes;
  const struct DeltaTableError *error;
} GenericOrError;

typedef void (*GenericErrorCallback)(const void *success, const struct DeltaTableError *fail);

typedef void (*TableEmptyCallback)(const struct DeltaTableError *fail);

typedef struct VacuumOptions {
  bool dry_run;
  uint64_t retention_hours;
  bool enforce_retention_duration;
  const struct Map *custom_metadata;
} VacuumOptions;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

const struct Map *map_new(const struct Runtime *runtime, uintptr_t capacity);

bool map_add(struct Map *map, const struct ByteArrayRef *key, const struct ByteArrayRef *value);

struct CancellationToken *cancellation_token_new(void);

void cancellation_token_cancel(struct CancellationToken *token);

void cancellation_token_free(struct CancellationToken *token);

void error_free(struct Runtime *_runtime, const struct DeltaTableError *error);

struct RuntimeOrFail runtime_new(const struct RuntimeOptions *options);

void runtime_free(struct Runtime *runtime);

void byte_array_free(struct Runtime *runtime, const struct ByteArray *bytes);

void map_free(struct Runtime *_runtime, const struct Map *map);

void dynamic_array_free(struct Runtime *runtime, const struct DynamicArray *array);

struct ByteArray *table_uri(const struct RawDeltaTable *table);

void table_free(struct RawDeltaTable *table);

void table_new(struct Runtime *runtime,
               const struct ByteArrayRef *table_uri,
               const struct TableOptions *table_options,
               TableNewCallback callback);

struct GenericOrError table_file_uris(struct Runtime *runtime, struct RawDeltaTable *table);

struct GenericOrError table_files(struct Runtime *runtime, struct RawDeltaTable *table);

void history(struct Runtime *runtime,
             struct RawDeltaTable *table,
             uintptr_t limit,
             GenericErrorCallback callback);

void table_update_incremental(struct Runtime *runtime,
                              struct RawDeltaTable *table,
                              TableEmptyCallback callback);

void table_load_version(struct Runtime *runtime,
                        struct RawDeltaTable *table,
                        int64_t version,
                        TableEmptyCallback callback);

void table_load_with_datetime(struct Runtime *runtime,
                              struct RawDeltaTable *table,
                              int64_t ts_milliseconds,
                              TableEmptyCallback callback);

void table_merge(struct Runtime *runtime,
                 struct RawDeltaTable *table,
                 int64_t version,
                 TableEmptyCallback callback);

void table_protocol(struct Runtime *runtime,
                    struct RawDeltaTable *table,
                    int64_t version,
                    TableEmptyCallback callback);

void table_restore(struct Runtime *runtime,
                   struct RawDeltaTable *table,
                   int64_t version,
                   TableEmptyCallback callback);

void table_update(struct Runtime *runtime,
                  struct RawDeltaTable *table,
                  int64_t version,
                  TableEmptyCallback callback);

/**
 * Must free the error, but there is no need to free the SerializedBuffer
 */
void table_schema(struct Runtime *runtime,
                  struct RawDeltaTable *table,
                  GenericErrorCallback callback);

void table_checkpoint(struct Runtime *runtime,
                      struct RawDeltaTable *table,
                      TableEmptyCallback callback);

void table_vacuum(struct Runtime *runtime,
                  struct RawDeltaTable *table,
                  const struct VacuumOptions *options,
                  GenericErrorCallback callback);

int64_t table_version(struct RawDeltaTable *table_handle);

void table_metadata(struct RawDeltaTable *table_handle, TableEmptyCallback callback);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus