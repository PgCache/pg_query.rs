//! Zero-copy access to the raw parse tree.
//!
//! Unlike [`crate::parse`], which serializes the parse tree to protobuf and
//! hands back bytes, this drives a caller-provided closure with the root
//! `List *` of the raw parse tree while it is still alive in the parser memory
//! context. The closure reads the C node tree directly (e.g. to build its own
//! AST) and must not retain any pointer into the tree past return.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::bindings::PgQueryError;
use crate::error::*;

#[repr(C)]
struct PgQueryScopedParseResult {
    error: *mut PgQueryError,
    stderr_buffer: *mut c_char,
    visitor_rc: c_int,
}

type RawVisitorFn = unsafe extern "C" fn(raw_parse_tree: *const c_void, user_ctx: *mut c_void) -> c_int;

extern "C" {
    fn pg_query_parse_raw_scoped(input: *const c_char, visit: Option<RawVisitorFn>, user_ctx: *mut c_void) -> PgQueryScopedParseResult;
    fn pg_query_free_scoped_parse_result(result: PgQueryScopedParseResult);
}

struct VisitorCtx<'a, R> {
    closure: Option<Box<dyn FnOnce(*const c_void) -> R + 'a>>,
    output: Option<std::thread::Result<R>>,
}

// Runs inside the C call; a panic here must not unwind across the FFI boundary,
// so we trap it and resume_unwind on the Rust side after C has returned.
unsafe extern "C" fn trampoline<R>(raw_parse_tree: *const c_void, user_ctx: *mut c_void) -> c_int {
    let ctx = unsafe { &mut *(user_ctx as *mut VisitorCtx<R>) };
    let Some(closure) = ctx.closure.take() else {
        return 1;
    };
    let result = catch_unwind(AssertUnwindSafe(|| closure(raw_parse_tree)));
    let rc = if result.is_ok() { 0 } else { 1 };
    ctx.output = Some(result);
    rc
}

/// Parse `statement` and invoke `visit` with the root `List *` of the raw parse
/// tree (as `*const c_void`) while it is alive. Returns the closure's value, or
/// [`Error::Parse`] on a parse error (in which case `visit` is not called).
pub fn parse_raw_scoped<R, F>(statement: &str, visit: F) -> Result<R>
where
    F: FnOnce(*const c_void) -> R,
{
    let input = CString::new(statement)?;
    let mut ctx: VisitorCtx<'_, R> = VisitorCtx { closure: Some(Box::new(visit)), output: None };

    let result = unsafe { pg_query_parse_raw_scoped(input.as_ptr(), Some(trampoline::<R>), &mut ctx as *mut VisitorCtx<'_, R> as *mut c_void) };

    let parse_error =
        if !result.error.is_null() { Some(unsafe { CStr::from_ptr((*result.error).message) }.to_string_lossy().to_string()) } else { None };
    unsafe { pg_query_free_scoped_parse_result(result) };

    if let Some(message) = parse_error {
        return Err(Error::Parse(message));
    }
    match ctx.output {
        Some(Ok(value)) => Ok(value),
        Some(Err(panic)) => std::panic::resume_unwind(panic),
        None => Err(Error::InvalidPointer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pg_nodes::{List, Node, NodeTag_T_RangeVar, NodeTag_T_SelectStmt, RangeVar, RawStmt, SelectStmt};

    /// Collect the `ptr_value` of each cell in a PG `List *` (the array-backed
    /// PG13+ representation). Empty for a NULL list (PG's NIL).
    unsafe fn list_ptrs(list: *const List) -> Vec<*mut c_void> {
        if list.is_null() {
            return Vec::new();
        }
        let len = (*list).length as usize;
        let elements = (*list).elements;
        (0..len).map(|i| (*elements.add(i)).ptr_value).collect()
    }

    #[test]
    fn parse_raw_scoped_walks_select_tree_directly() {
        let (statements, targets, relname) = parse_raw_scoped("SELECT a, b FROM t WHERE x = 1", |tree| unsafe {
            let stmts = list_ptrs(tree as *const List);
            let raw = stmts[0] as *const RawStmt;
            let stmt_node = (*raw).stmt as *const Node;
            assert_eq!((*stmt_node).type_, NodeTag_T_SelectStmt, "top-level node should be a SelectStmt");

            let select = stmt_node as *const SelectStmt;
            let targets = list_ptrs((*select).targetList).len();

            let from = list_ptrs((*select).fromClause);
            let range_var = from[0] as *const RangeVar;
            assert_eq!((*range_var).type_, NodeTag_T_RangeVar);
            let relname = CStr::from_ptr((*range_var).relname).to_string_lossy().into_owned();

            (stmts.len(), targets, relname)
        })
        .expect("parse and walk select");

        assert_eq!(statements, 1);
        assert_eq!(targets, 2);
        assert_eq!(relname, "t");
    }

    #[test]
    fn parse_raw_scoped_invokes_visitor_for_valid_select() {
        let tree_non_null = parse_raw_scoped("SELECT 1", |tree| !tree.is_null()).expect("parse valid select");
        assert!(tree_non_null, "raw parse tree should be non-null for a valid statement");
    }

    #[test]
    fn parse_raw_scoped_returns_parse_error_and_skips_visitor() {
        let mut visited = false;
        let result = parse_raw_scoped("SELECT FROM WHERE", |_| {
            visited = true;
        });
        assert!(matches!(result, Err(Error::Parse(_))), "expected parse error, got {result:?}");
        assert!(!visited, "visitor must not run on parse error");
    }
}
