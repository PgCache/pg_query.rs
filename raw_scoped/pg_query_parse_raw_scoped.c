#include "pg_query.h"
#include "pg_query_internal.h"
#include "pg_query_raw_scoped.h"

#include "parser/parser.h"

// Parse `input` to the raw (un-analyzed) parse tree and invoke `visit` with the
// root `List *` of RawStmt nodes while the tree is still alive in the parser
// memory context. The caller's visitor reads the node tree directly (e.g. to
// build its own AST) and must NOT retain any pointer into the tree past return.
// On parse error the visitor is not called; `error`/`stderr_buffer` are malloc'd
// and outlive the memory context (free via pg_query_free_scoped_parse_result).
PgQueryScopedParseResult pg_query_parse_raw_scoped(const char *input, PgQueryRawVisitor visit, void *user_ctx)
{
	MemoryContext ctx = NULL;
	PgQueryInternalParsetreeAndError parsetree_and_error;
	PgQueryScopedParseResult result = {0};

	ctx = pg_query_enter_memory_context();

	parsetree_and_error = pg_query_raw_parse(input, PG_QUERY_PARSE_DEFAULT);

	result.stderr_buffer = parsetree_and_error.stderr_buffer;
	result.error = parsetree_and_error.error;

	if (parsetree_and_error.error == NULL && visit != NULL) {
		result.visitor_rc = visit(parsetree_and_error.tree, user_ctx);
	}

	pg_query_exit_memory_context(ctx);

	return result;
}

void pg_query_free_scoped_parse_result(PgQueryScopedParseResult result)
{
	if (result.error) {
		pg_query_free_error(result.error);
	}

	free(result.stderr_buffer);
}
