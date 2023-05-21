use crate::c_api::hash::{with_hash, with_hash_mut};
use crate::c_api::other::with_other_mut;
use crate::c_api::pool::with_pool;
use crate::c_api::xbuf::XBuf;
use crate::c_api::{CiteNumber, FindCiteLocs, HashPointer2, StrIlk, StrNumber};
use std::cell::RefCell;

pub const MAX_CITES: usize = 750;

pub struct CiteInfo {
    cite_list: XBuf<StrNumber>,
    cite_info: XBuf<StrNumber>,
    type_list: XBuf<HashPointer2>,
    entry_exists: XBuf<bool>,
    cite_ptr: CiteNumber,

    entry_cite_ptr: CiteNumber,
    num_cites: CiteNumber,
    old_num_cites: CiteNumber,
    all_marker: CiteNumber,
}

impl CiteInfo {
    fn new() -> CiteInfo {
        CiteInfo {
            cite_list: XBuf::new(MAX_CITES),
            cite_info: XBuf::new(MAX_CITES),
            type_list: XBuf::new(MAX_CITES),
            entry_exists: XBuf::new(MAX_CITES),
            cite_ptr: 0,
            entry_cite_ptr: 0,
            num_cites: 0,
            old_num_cites: 0,
            all_marker: 0,
        }
    }

    fn grow(&mut self) {
        self.cite_list.grow(MAX_CITES);
        self.cite_info.grow(MAX_CITES);
        self.type_list.grow(MAX_CITES);
        self.entry_exists.grow(MAX_CITES);
    }

    pub fn get_cite(&self, offset: usize) -> StrNumber {
        self.cite_list[offset]
    }

    pub fn set_cite(&mut self, offset: usize, num: StrNumber) {
        self.cite_list[offset] = num;
    }

    pub fn get_info(&self, offset: usize) -> StrNumber {
        self.cite_info[offset]
    }

    pub fn set_info(&mut self, offset: usize, num: StrNumber) {
        self.cite_info[offset] = num;
    }

    pub fn get_type(&self, offset: usize) -> HashPointer2 {
        self.type_list[offset]
    }

    pub fn set_type(&mut self, offset: usize, ty: HashPointer2) {
        self.type_list[offset] = ty;
    }

    pub fn get_exists(&self, offset: usize) -> bool {
        self.entry_exists[offset]
    }

    pub fn set_exists(&mut self, offset: usize, exists: bool) {
        self.entry_exists[offset] = exists;
    }

    pub fn ptr(&self) -> CiteNumber {
        self.cite_ptr
    }

    pub fn set_ptr(&mut self, ptr: CiteNumber) {
        self.cite_ptr = ptr;
    }

    pub fn num_cites(&self) -> CiteNumber {
        self.num_cites
    }
}

thread_local! {
    pub static CITE_INFO: RefCell<CiteInfo> = RefCell::new(CiteInfo::new());
}

pub fn reset() {
    CITE_INFO.with(|ci| *ci.borrow_mut() = CiteInfo::new());
}

pub fn with_cites<T>(f: impl FnOnce(&CiteInfo) -> T) -> T {
    CITE_INFO.with(|ci| f(&ci.borrow()))
}

pub fn with_cites_mut<T>(f: impl FnOnce(&mut CiteInfo) -> T) -> T {
    CITE_INFO.with(|ci| f(&mut ci.borrow_mut()))
}

#[no_mangle]
pub extern "C" fn quick_sort(left_end: CiteNumber, right_end: CiteNumber) {
    with_cites_mut(|cites| cites.cite_info[left_end as usize..right_end as usize].sort())
}

#[no_mangle]
pub extern "C" fn cite_list(num: CiteNumber) -> StrNumber {
    with_cites(|cites| cites.get_cite(num as usize))
}

#[no_mangle]
pub extern "C" fn set_cite_list(num: CiteNumber, str: StrNumber) {
    with_cites_mut(|cites| cites.set_cite(num as usize, str))
}

#[no_mangle]
pub extern "C" fn cite_ptr() -> CiteNumber {
    with_cites(|cites| cites.ptr())
}

#[no_mangle]
pub extern "C" fn set_cite_ptr(num: CiteNumber) {
    with_cites_mut(|cites| cites.set_ptr(num))
}

#[no_mangle]
pub extern "C" fn check_cite_overflow(last_cite: CiteNumber) {
    with_cites_mut(|cites| {
        if last_cite as usize == cites.cite_list.len() {
            cites.grow();
        }
    })
}

#[no_mangle]
pub extern "C" fn max_cites() -> usize {
    with_cites(|cites| cites.cite_list.len())
}

#[no_mangle]
pub extern "C" fn cite_info(num: CiteNumber) -> StrNumber {
    with_cites(|cites| cites.get_info(num as usize))
}

#[no_mangle]
pub extern "C" fn set_cite_info(num: CiteNumber, info: StrNumber) {
    with_cites_mut(|cites| cites.set_info(num as usize, info))
}

#[no_mangle]
pub extern "C" fn type_list(num: CiteNumber) -> HashPointer2 {
    with_cites(|cites| cites.get_type(num as usize))
}

#[no_mangle]
pub extern "C" fn set_type_list(num: CiteNumber, ty: HashPointer2) {
    with_cites_mut(|cites| cites.set_type(num as usize, ty))
}

#[no_mangle]
pub extern "C" fn entry_exists(num: CiteNumber) -> bool {
    with_cites(|cites| cites.get_exists(num as usize))
}

#[no_mangle]
pub extern "C" fn set_entry_exists(num: CiteNumber, exists: bool) {
    with_cites_mut(|cites| cites.set_exists(num as usize, exists))
}

#[no_mangle]
pub extern "C" fn entry_cite_ptr() -> CiteNumber {
    with_cites(|cites| cites.entry_cite_ptr)
}

#[no_mangle]
pub extern "C" fn set_entry_cite_ptr(val: CiteNumber) {
    with_cites_mut(|cites| cites.entry_cite_ptr = val)
}

#[no_mangle]
pub extern "C" fn num_cites() -> CiteNumber {
    with_cites(|cites| cites.num_cites)
}

#[no_mangle]
pub extern "C" fn set_num_cites(val: CiteNumber) {
    with_cites_mut(|cites| cites.num_cites = val)
}

#[no_mangle]
pub extern "C" fn old_num_cites() -> CiteNumber {
    with_cites(|cites| cites.old_num_cites)
}

#[no_mangle]
pub extern "C" fn set_old_num_cites(val: CiteNumber) {
    with_cites_mut(|cites| cites.old_num_cites = val)
}

#[no_mangle]
pub extern "C" fn all_marker() -> CiteNumber {
    with_cites(|cites| cites.all_marker)
}

#[no_mangle]
pub extern "C" fn set_all_marker(val: CiteNumber) {
    with_cites_mut(|cites| cites.all_marker = val)
}

#[no_mangle]
pub extern "C" fn add_database_cite(
    new_cite: CiteNumber,
    cite_loc: CiteNumber,
    lc_cite_loc: CiteNumber,
) -> CiteNumber {
    with_cites_mut(|cites| {
        if new_cite as usize == cites.cite_list.len() {
            cites.grow();
        }
        with_other_mut(|other| other.check_field_overflow(other.num_fields() * (new_cite + 1)));

        with_hash_mut(|hash| {
            cites.set_cite(new_cite as usize, hash.text(cite_loc as usize));
            hash.set_ilk_info(cite_loc as usize, new_cite);
            hash.set_ilk_info(lc_cite_loc as usize, cite_loc);
        });
    });
    new_cite + 1
}

#[no_mangle]
pub extern "C" fn find_cite_locs_for_this_cite_key(cite_str: StrNumber) -> FindCiteLocs {
    with_pool(|pool| {
        let val = pool.get_str(cite_str as usize);

        let (cite_hash, lc_cite_hash) = with_hash(|hash| {
            let cite_hash = pool.lookup_str(hash, val, StrIlk::Cite);
            let lc_cite_hash = pool.lookup_str(hash, &val.to_ascii_lowercase(), StrIlk::LcCite);
            (cite_hash, lc_cite_hash)
        });

        FindCiteLocs {
            cite_loc: cite_hash.loc,
            cite_found: cite_hash.exists,
            lc_cite_loc: lc_cite_hash.loc,
            lc_found: lc_cite_hash.exists,
        }
    })
}
