/* Bindgen input for the PostgreSQL raw parse-tree node structs.
 * Pulled in by build.rs to generate Rust layouts for the nodes that appear in
 * raw SELECT parse trees, so callers of pg_query_parse_raw_scoped can read the
 * tree directly. Include paths point at the vendored libpg_query PG headers. */
#include "postgres.h"
#include "nodes/pg_list.h"
#include "nodes/nodes.h"
#include "nodes/value.h"
#include "nodes/primnodes.h"
#include "nodes/parsenodes.h"
