#ifndef PG_QUERY_RAW_SCOPED_H
#define PG_QUERY_RAW_SCOPED_H

#include "pg_query.h"

// Visitor invoked with the root `List *` of the raw parse tree (passed as an
// opaque pointer so this header carries no PostgreSQL node types). Return value
// is surfaced to the caller via PgQueryScopedParseResult.visitor_rc.
typedef int (*PgQueryRawVisitor)(const void *raw_parse_tree, void *user_ctx);

typedef struct {
	PgQueryError *error;
	char *stderr_buffer;
	int visitor_rc;
} PgQueryScopedParseResult;

PgQueryScopedParseResult pg_query_parse_raw_scoped(const char *input, PgQueryRawVisitor visit, void *user_ctx);
void pg_query_free_scoped_parse_result(PgQueryScopedParseResult result);

#endif
