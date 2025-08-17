extern crate bex as bex_rs;
use bex_rs::{Base, BddBase, nid::{NID, I, O}, vid::VID};
use std::sync::{Arc, Mutex};

#[repr(C)]
pub struct bex_mgr_t {
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
pub extern "C" fn bex_mgr_new() -> *mut bex_mgr_t {
    let base = Box::into_raw(Box::new(Arc::new(Mutex::new(BddBase::new()))));
    Box::into_raw(Box::new(bex_mgr_t {
        base: base as *mut std::ffi::c_void,
    }))
}

#[no_mangle]
pub extern "C" fn bex_mgr_free(mgr: *mut bex_mgr_t) {
    if !mgr.is_null() {
        unsafe {
            let base_ptr = (*mgr).base as *mut Arc<Mutex<BddBase>>;
            drop(Box::from_raw(base_ptr));
            drop(Box::from_raw(mgr));
        }
    }
}

#[no_mangle]
pub extern "C" fn bex_top(mgr: *mut bex_mgr_t) -> bex_nid_t {
    bex_nid_t { nid: I._to_u64() }
}

#[no_mangle]
pub extern "C" fn bex_bot(mgr: *mut bex_mgr_t) -> bex_nid_t {
    bex_nid_t { nid: O._to_u64() }
}

#[no_mangle]
pub extern "C" fn bex_ithvar(mgr: *mut bex_mgr_t, vid: bex_vid_t) -> bex_nid_t {
    bex_nid_t { nid: NID::from_vid(VID::var(vid.vid))._to_u64() }
}

#[no_mangle]
pub extern "C" fn bex_and(mgr: *mut bex_mgr_t, a: bex_nid_t, b: bex_nid_t) -> bex_nid_t {
    unsafe {
        let base_ptr = (*mgr).base as *mut Arc<Mutex<BddBase>>;
        let mut base = (*base_ptr).lock().unwrap();
        let nid_a = NID::_from_u64(a.nid);
        let nid_b = NID::_from_u64(b.nid);
        bex_nid_t { nid: base.and(nid_a, nid_b)._to_u64() }
    }
}

#[no_mangle]
pub extern "C" fn bex_or(mgr: *mut bex_mgr_t, a: bex_nid_t, b: bex_nid_t) -> bex_nid_t {
    unsafe {
        let base_ptr = (*mgr).base as *mut Arc<Mutex<BddBase>>;
        let mut base = (*base_ptr).lock().unwrap();
        let nid_a = NID::_from_u64(a.nid);
        let nid_b = NID::_from_u64(b.nid);
        bex_nid_t { nid: base.or(nid_a, nid_b)._to_u64() }
    }
}

#[no_mangle]
pub extern "C" fn bex_xor(mgr: *mut bex_mgr_t, a: bex_nid_t, b: bex_nid_t) -> bex_nid_t {
    unsafe {
        let base_ptr = (*mgr).base as *mut Arc<Mutex<BddBase>>;
        let mut base = (*base_ptr).lock().unwrap();
        let nid_a = NID::_from_u64(a.nid);
        let nid_b = NID::_from_u64(b.nid);
        bex_nid_t { nid: base.xor(nid_a, nid_b)._to_u64() }
    }
}

#[no_mangle]
pub extern "C" fn bex_ite(mgr: *mut bex_mgr_t, i: bex_nid_t, t: bex_nid_t, e: bex_nid_t) -> bex_nid_t {
    unsafe {
        let base_ptr = (*mgr).base as *mut Arc<Mutex<BddBase>>;
        let mut base = (*base_ptr).lock().unwrap();
        let nid_i = NID::_from_u64(i.nid);
        let nid_t = NID::_from_u64(t.nid);
        let nid_e = NID::_from_u64(e.nid);
        bex_nid_t { nid: base.ite(nid_i, nid_t, nid_e)._to_u64() }
    }
}

#[no_mangle]
pub extern "C" fn bex_node_count(mgr: *mut bex_mgr_t, n: bex_nid_t) -> u64 {
    unsafe {
        let base_ptr = (*mgr).base as *mut Arc<Mutex<BddBase>>;
        let base = (*base_ptr).lock().unwrap();
        let nid = NID::_from_u64(n.nid);
        base.node_count(nid) as u64
    }
}

#[no_mangle]
pub extern "C" fn bex_solution_count(mgr: *mut bex_mgr_t, n: bex_nid_t) -> u64 {
    unsafe {
        let base_ptr = (*mgr).base as *mut Arc<Mutex<BddBase>>;
        let mut base = (*base_ptr).lock().unwrap();
        let nid = NID::_from_u64(n.nid);
        base.solution_count(nid)
    }
}
