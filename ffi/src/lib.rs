extern crate bex as bex_rs;
use bex_rs::{Base, BddBase, ast::ASTBase, nid::{NID, I, O}, vid::VID};
use std::sync::{Arc, Mutex};

macro_rules! bex_bdd_op_body {
    ($bdd:ident, $method:ident, $($param:ident),*) => {
        unsafe {
            let base_ptr = (*$bdd).base as *mut Arc<Mutex<BddBase>>;
            let mut base = (*base_ptr).lock().unwrap();
            $(let $param = NID::_from_u64($param.nid);)*
            bex_nid_t { nid: base.$method($($param),*)._to_u64() }
        }
    };
}

macro_rules! bex_ast_op_body {
    ($ast:ident, $method:ident, $($param:ident),*) => {
        unsafe {
            let base_ptr = (*$ast).base as *mut Arc<Mutex<ASTBase>>;
            let mut base = (*base_ptr).lock().unwrap();
            $(let $param = NID::_from_u64($param.nid);)*
            bex_nid_t { nid: base.$method($($param),*)._to_u64() }
        }
    };
}

#[repr(C)]
pub struct bex_bdd_t {
    base: *mut std::ffi::c_void,
}

#[repr(C)]
pub struct bex_ast_t {
    base: *mut std::ffi::c_void,
}

#[repr(C)]
pub struct bex_nid_t {
    nid: u64,
}

#[repr(C)]
pub struct bex_vid_t {
    vid: u32,
}

#[no_mangle]
pub extern "C" fn bex_bdd_new() -> *mut bex_bdd_t {
    let base = Box::into_raw(Box::new(Arc::new(Mutex::new(BddBase::new()))));
    Box::into_raw(Box::new(bex_bdd_t {
        base: base as *mut std::ffi::c_void,
    }))
}

#[no_mangle]
pub extern "C" fn bex_bdd_free(bdd: *mut bex_bdd_t) {
    if !bdd.is_null() {
        unsafe {
            let base_ptr = (*bdd).base as *mut Arc<Mutex<BddBase>>;
            drop(Box::from_raw(base_ptr));
            drop(Box::from_raw(bdd));
        }
    }
}

#[no_mangle]
pub extern "C" fn bex_ast_new() -> *mut bex_ast_t {
    let base = Box::into_raw(Box::new(Arc::new(Mutex::new(ASTBase::new()))));
    Box::into_raw(Box::new(bex_ast_t {
        base: base as *mut std::ffi::c_void,
    }))
}

#[no_mangle]
pub extern "C" fn bex_ast_free(ast: *mut bex_ast_t) {
    if !ast.is_null() {
        unsafe {
            let base_ptr = (*ast).base as *mut Arc<Mutex<ASTBase>>;
            drop(Box::from_raw(base_ptr));
            drop(Box::from_raw(ast));
        }
    }
}

#[no_mangle]
pub extern "C" fn bex_top() -> bex_nid_t {
    bex_nid_t { nid: I._to_u64() }
}

#[no_mangle]
pub extern "C" fn bex_bot() -> bex_nid_t {
    bex_nid_t { nid: O._to_u64() }
}

#[no_mangle]
pub extern "C" fn bex_ithvar(vid: bex_vid_t) -> bex_nid_t {
    bex_nid_t { nid: NID::from_vid(VID::var(vid.vid))._to_u64() }
}

#[no_mangle]
pub extern "C" fn bex_bdd_and(bdd: *mut bex_bdd_t, a: bex_nid_t, b: bex_nid_t) -> bex_nid_t {
    bex_bdd_op_body!(bdd, and, a, b)
}

#[no_mangle]
pub extern "C" fn bex_bdd_or(bdd: *mut bex_bdd_t, a: bex_nid_t, b: bex_nid_t) -> bex_nid_t {
    bex_bdd_op_body!(bdd, or, a, b)
}

#[no_mangle]
pub extern "C" fn bex_bdd_xor(bdd: *mut bex_bdd_t, a: bex_nid_t, b: bex_nid_t) -> bex_nid_t {
    bex_bdd_op_body!(bdd, xor, a, b)
}

#[no_mangle]
pub extern "C" fn bex_bdd_ite(bdd: *mut bex_bdd_t, i: bex_nid_t, t: bex_nid_t, e: bex_nid_t) -> bex_nid_t {
    bex_bdd_op_body!(bdd, ite, i, t, e)
}

#[no_mangle]
pub extern "C" fn bex_ast_and(ast: *mut bex_ast_t, a: bex_nid_t, b: bex_nid_t) -> bex_nid_t {
    bex_ast_op_body!(ast, and, a, b)
}

#[no_mangle]
pub extern "C" fn bex_ast_or(ast: *mut bex_ast_t, a: bex_nid_t, b: bex_nid_t) -> bex_nid_t {
    bex_ast_op_body!(ast, or, a, b)
}

#[no_mangle]
pub extern "C" fn bex_ast_xor(ast: *mut bex_ast_t, a: bex_nid_t, b: bex_nid_t) -> bex_nid_t {
    bex_ast_op_body!(ast, xor, a, b)
}

#[no_mangle]
pub extern "C" fn bex_not(n: bex_nid_t) -> bex_nid_t {
    let nid = NID::_from_u64(n.nid);
    bex_nid_t { nid: (!nid)._to_u64() }
}

#[no_mangle]
pub extern "C" fn bex_bdd_node_count(bdd: *mut bex_bdd_t, n: bex_nid_t) -> u64 {
    unsafe {
        let base_ptr = (*bdd).base as *mut Arc<Mutex<BddBase>>;
        let base = (*base_ptr).lock().unwrap();
        let nid = NID::_from_u64(n.nid);
        base.node_count(nid) as u64
    }
}

#[no_mangle]
pub extern "C" fn bex_bdd_solution_count(bdd: *mut bex_bdd_t, n: bex_nid_t) -> u64 {
    unsafe {
        let base_ptr = (*bdd).base as *mut Arc<Mutex<BddBase>>;
        let mut base = (*base_ptr).lock().unwrap();
        let nid = NID::_from_u64(n.nid);
        base.solution_count(nid)
    }
}
