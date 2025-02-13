use crate::{
    c_api::{
        buffer::{with_buffers, BufTy},
        entries::{with_entries_mut, ENT_STR_SIZE},
        global::GLOB_STR_SIZE,
        hash,
        hash::{with_hash, with_hash_mut, FnClass, HashData},
        log::{print_overflow, write_logs},
        other::with_other_mut,
        xbuf::XBuf,
        ASCIICode, Bibtex, BufPointer, CResult, CResultLookup, CResultStr, HashPointer, LookupRes,
        PoolPointer, StrIlk, StrNumber,
    },
    BibtexError,
};
use std::cell::RefCell;

const POOL_SIZE: usize = 65000;
pub(crate) const MAX_STRINGS: usize = 35307;

#[derive(Debug, PartialEq)]
pub enum LookupErr {
    Invalid,
    DoesntExist,
}

pub struct StringPool {
    strings: XBuf<u8>,
    // Stores string starting locations in the string pool
    // length of string `s` is offsets[s + 1] - offsets[s]
    offsets: XBuf<usize>,
    pool_ptr: PoolPointer,
    str_ptr: StrNumber,
}

impl StringPool {
    fn new() -> StringPool {
        StringPool {
            strings: XBuf::new(POOL_SIZE),
            offsets: XBuf::new(MAX_STRINGS),
            pool_ptr: 0,
            str_ptr: 1,
        }
    }

    pub fn try_get_str(&self, s: usize) -> Result<&[u8], LookupErr> {
        // TODO: Why plus three? Should probably find if somewhere relies on that
        if s == 0 || s >= self.str_ptr + 3 {
            Err(LookupErr::DoesntExist)
        } else if s >= MAX_STRINGS {
            Err(LookupErr::Invalid)
        } else {
            Ok(&self.strings[self.offsets[s]..self.offsets[s + 1]])
        }
    }

    pub fn get_str(&self, s: usize) -> &[u8] {
        self.try_get_str(s).unwrap_or_else(|e| match e {
            LookupErr::DoesntExist => panic!("String number {} doesn't exist", s),
            LookupErr::Invalid => panic!("Invalid string number {}", s),
        })
    }

    pub fn grow(&mut self) {
        self.strings.grow(POOL_SIZE);
    }

    /// Used while defining strings - declare the current `pool_ptr` as the end of the current
    /// string, increment the `str_ptr`, and return the new string's `StrNumber`
    fn make_string(&mut self) -> CResultStr {
        if self.str_ptr == MAX_STRINGS {
            print_overflow();
            write_logs(&format!("number of strings {}\n", MAX_STRINGS));
            return CResultStr::Error;
        }
        self.str_ptr += 1;
        self.offsets[self.str_ptr] = self.pool_ptr;
        CResultStr::Ok(self.str_ptr - 1)
    }

    fn hash_str(hash: &HashData, str: &[ASCIICode]) -> usize {
        let prime = hash.prime();
        str.iter()
            .fold(0, |acc, &c| ((2 * acc) + c as usize) % prime)
    }

    pub fn lookup_str(&self, hash: &HashData, str: &[ASCIICode], ilk: StrIlk) -> LookupRes {
        let h = Self::hash_str(hash, str);
        let mut p = h as HashPointer + hash::HASH_BASE as HashPointer;

        loop {
            let existing = hash.text(p);

            if existing > 0 && self.get_str(existing) == str && hash.hash_ilk(p) == ilk {
                return LookupRes {
                    loc: p,
                    exists: true,
                };
            }

            if hash.next(p) == 0 {
                return LookupRes {
                    loc: p,
                    exists: false,
                };
            }
            p = hash.next(p);
        }
    }

    /// Lookup a string, inserting it if it isn't found. Note that this returns `Ok` whether the
    /// string is found or not, only returning `Err` if a called function fails.
    pub(crate) fn lookup_str_insert(
        &mut self,
        hash: &mut HashData,
        str: &[ASCIICode],
        ilk: StrIlk,
    ) -> Result<LookupRes, BibtexError> {
        let h = Self::hash_str(hash, str);
        let mut str_num = 0;
        let mut p = (h + hash::HASH_BASE) as HashPointer;

        loop {
            let existing = hash.text(p);
            if existing > 0 && self.try_get_str(existing) == Ok(str) {
                if hash.hash_ilk(p) == ilk {
                    return Ok(LookupRes {
                        loc: p,
                        exists: true,
                    });
                } else {
                    str_num = existing;
                }
            }

            if hash.next(p) == 0 {
                if existing > 0 {
                    loop {
                        if hash.used() == hash::HASH_BASE {
                            print_overflow();
                            write_logs(&format!("hash size {}\n", hash::HASH_SIZE));
                            return Err(BibtexError);
                        }
                        hash.set_used(hash.used() - 1);

                        if hash.text(hash.used()) == 0 {
                            break;
                        }
                    }
                    hash.set_next(p, hash.used());
                    p = hash.used();
                }

                if str_num > 0 {
                    hash.set_text(p, str_num);
                } else {
                    while self.pool_ptr + str.len() > self.strings.len() {
                        self.grow();
                    }
                    self.strings[self.pool_ptr..self.pool_ptr + str.len()].copy_from_slice(str);
                    self.pool_ptr += str.len();

                    match self.make_string() {
                        CResultStr::Ok(str) => hash.set_text(p, str),
                        _ => return Err(BibtexError),
                    }
                }

                hash.set_hash_ilk(p, ilk);

                return Ok(LookupRes {
                    loc: p,
                    exists: false,
                });
            }

            p = hash.next(p);
        }
    }
}

thread_local! {
    static STRING_POOL: RefCell<StringPool> = RefCell::new(StringPool::new());
}

pub(crate) fn reset() {
    STRING_POOL.with(|pool| *pool.borrow_mut() = StringPool::new());
}

pub fn with_pool<T>(f: impl FnOnce(&StringPool) -> T) -> T {
    STRING_POOL.with(|pool| f(&pool.borrow()))
}

pub fn with_pool_mut<T>(f: impl FnOnce(&mut StringPool) -> T) -> T {
    STRING_POOL.with(|pool| f(&mut pool.borrow_mut()))
}

#[no_mangle]
pub extern "C" fn bib_str_eq_str(s1: StrNumber, s2: StrNumber) -> bool {
    with_pool(|pool| pool.get_str(s1) == pool.get_str(s2))
}

#[no_mangle]
pub extern "C" fn pool_overflow() {
    with_pool_mut(|pool| pool.grow());
}

#[no_mangle]
pub extern "C" fn bib_str_pool(idx: PoolPointer) -> ASCIICode {
    with_pool(|pool| pool.strings[idx])
}

#[no_mangle]
pub extern "C" fn bib_set_str_pool(idx: PoolPointer, code: ASCIICode) {
    with_pool_mut(|pool| pool.strings[idx] = code)
}

#[no_mangle]
pub extern "C" fn bib_str_ptr() -> StrNumber {
    with_pool(|pool| pool.str_ptr)
}

#[no_mangle]
pub extern "C" fn bib_set_str_ptr(ptr: StrNumber) {
    with_pool_mut(|pool| pool.str_ptr = ptr);
}

#[no_mangle]
pub extern "C" fn bib_str_start(s: StrNumber) -> PoolPointer {
    with_pool(|pool| pool.offsets[s])
}

#[no_mangle]
pub extern "C" fn bib_set_str_start(s: StrNumber, ptr: PoolPointer) {
    with_pool_mut(|pool| pool.offsets[s] = ptr)
}

#[no_mangle]
pub extern "C" fn bib_pool_size() -> usize {
    with_pool(|pool| pool.strings.len())
}

#[no_mangle]
pub extern "C" fn bib_max_strings() -> usize {
    MAX_STRINGS
}

#[no_mangle]
pub extern "C" fn bib_pool_ptr() -> PoolPointer {
    with_pool(|pool| pool.pool_ptr)
}

#[no_mangle]
pub extern "C" fn bib_set_pool_ptr(ptr: PoolPointer) {
    with_pool_mut(|pool| pool.pool_ptr = ptr)
}

#[no_mangle]
pub extern "C" fn bib_make_string() -> CResultStr {
    with_pool_mut(|pool| pool.make_string())
}

#[no_mangle]
pub extern "C" fn str_lookup(
    buf: BufTy,
    ptr: BufPointer,
    len: BufPointer,
    ilk: StrIlk,
    insert: bool,
) -> CResultLookup {
    with_buffers(|buffers| {
        let str = &buffers.buffer(buf)[ptr..(ptr + len)];
        if insert {
            with_hash_mut(|hash| {
                with_pool_mut(|pool| pool.lookup_str_insert(hash, str, ilk).into())
            })
        } else {
            with_hash(|hash| with_pool(|pool| CResultLookup::Ok(pool.lookup_str(hash, str, ilk))))
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn pre_def_certain_strings(ctx: *mut Bibtex) -> CResult {
    let ctx = &mut *ctx;
    let res = with_hash_mut(|hash| {
        with_pool_mut(|pool| {
            let res = pool.lookup_str_insert(hash, b".aux", StrIlk::FileExt)?;
            ctx.s_aux_extension = hash.text(res.loc);

            let res = pool.lookup_str_insert(hash, b"\\citation", StrIlk::AuxCommand)?;
            hash.set_ilk_info(res.loc, 2);
            let res = pool.lookup_str_insert(hash, b"\\bibdata", StrIlk::AuxCommand)?;
            hash.set_ilk_info(res.loc, 0);
            let res = pool.lookup_str_insert(hash, b"\\bibstyle", StrIlk::AuxCommand)?;
            hash.set_ilk_info(res.loc, 1);
            let res = pool.lookup_str_insert(hash, b"\\@input", StrIlk::AuxCommand)?;
            hash.set_ilk_info(res.loc, 3);

            let res = pool.lookup_str_insert(hash, b"entry", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 0);
            let res = pool.lookup_str_insert(hash, b"execute", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 1);
            let res = pool.lookup_str_insert(hash, b"function", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 2);
            let res = pool.lookup_str_insert(hash, b"integers", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 3);
            let res = pool.lookup_str_insert(hash, b"iterate", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 4);
            let res = pool.lookup_str_insert(hash, b"macro", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 5);
            let res = pool.lookup_str_insert(hash, b"read", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 6);
            let res = pool.lookup_str_insert(hash, b"reverse", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 7);
            let res = pool.lookup_str_insert(hash, b"sort", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 8);
            let res = pool.lookup_str_insert(hash, b"strings", StrIlk::BstCommand)?;
            hash.set_ilk_info(res.loc, 9);

            let res = pool.lookup_str_insert(hash, b"comment", StrIlk::BibCommand)?;
            hash.set_ilk_info(res.loc, 0);
            let res = pool.lookup_str_insert(hash, b"preamble", StrIlk::BibCommand)?;
            hash.set_ilk_info(res.loc, 1);
            let res = pool.lookup_str_insert(hash, b"string", StrIlk::BibCommand)?;
            hash.set_ilk_info(res.loc, 2);

            let mut build_in = |pds: &[ASCIICode], blt_in_num| {
                let res = pool.lookup_str_insert(hash, pds, StrIlk::BstFn)?;
                hash.set_ty(res.loc, FnClass::Builtin);
                hash.set_ilk_info(res.loc, blt_in_num);
                Ok(res.loc)
            };

            build_in(b"=", 0)?;
            build_in(b">", 1)?;
            build_in(b"<", 2)?;
            build_in(b"+", 3)?;
            build_in(b"-", 4)?;
            build_in(b"*", 5)?;
            build_in(b":=", 6)?;
            build_in(b"add.period$", 7)?;
            build_in(b"call.type$", 8)?;
            build_in(b"change.case$", 9)?;
            build_in(b"chr.to.int$", 10)?;
            build_in(b"cite$", 11)?;
            build_in(b"duplicate$", 12)?;
            build_in(b"empty$", 13)?;
            build_in(b"format.name$", 14)?;
            build_in(b"if$", 15)?;
            build_in(b"int.to.chr$", 16)?;
            build_in(b"int.to.str$", 17)?;
            build_in(b"missing$", 18)?;
            build_in(b"newline$", 19)?;
            build_in(b"num.names$", 20)?;
            build_in(b"pop$", 21)?;
            build_in(b"preamble$", 22)?;
            build_in(b"purify$", 23)?;
            build_in(b"quote$", 24)?;
            let skip_loc = build_in(b"skip$", 25)?;
            build_in(b"stack$", 26)?;
            build_in(b"substring$", 27)?;
            build_in(b"swap$", 28)?;
            build_in(b"text.length$", 29)?;
            build_in(b"text.prefix$", 30)?;
            build_in(b"top$", 31)?;
            build_in(b"type$", 32)?;
            build_in(b"warning$", 33)?;
            build_in(b"while$", 34)?;
            build_in(b"width$", 35)?;
            build_in(b"write$", 36)?;

            let res = pool.lookup_str_insert(hash, b"", StrIlk::Text)?;
            hash.set_ty(res.loc, FnClass::StrLit);
            ctx.s_null = hash.text(res.loc);
            let res = pool.lookup_str_insert(hash, b"default.type", StrIlk::Text)?;
            hash.set_ty(res.loc, FnClass::StrLit);
            ctx.s_default = hash.text(res.loc);
            ctx.b_default = skip_loc;

            let res = pool.lookup_str_insert(hash, b"i", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 0);
            let res = pool.lookup_str_insert(hash, b"j", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 1);
            let res = pool.lookup_str_insert(hash, b"oe", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 2);
            let res = pool.lookup_str_insert(hash, b"OE", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 3);
            let res = pool.lookup_str_insert(hash, b"ae", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 4);
            let res = pool.lookup_str_insert(hash, b"AE", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 5);
            let res = pool.lookup_str_insert(hash, b"aa", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 6);
            let res = pool.lookup_str_insert(hash, b"AA", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 7);
            let res = pool.lookup_str_insert(hash, b"o", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 8);
            let res = pool.lookup_str_insert(hash, b"O", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 9);
            let res = pool.lookup_str_insert(hash, b"l", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 10);
            let res = pool.lookup_str_insert(hash, b"L", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 11);
            let res = pool.lookup_str_insert(hash, b"ss", StrIlk::ControlSeq)?;
            hash.set_ilk_info(res.loc, 12);

            with_other_mut(|other| {
                let res = pool.lookup_str_insert(hash, b"crossref", StrIlk::BstFn)?;
                let num_fields = other.num_fields();
                hash.set_ty(res.loc, FnClass::Field);
                hash.set_ilk_info(res.loc, num_fields as i32);
                other.set_crossref_num(num_fields);
                other.set_num_fields(num_fields + 1);
                other.set_pre_defined_fields(num_fields + 1);
                Ok(())
            })?;

            with_entries_mut(|entries| {
                let res = pool.lookup_str_insert(hash, b"sort.key$", StrIlk::BstFn)?;
                hash.set_ty(res.loc, FnClass::StrEntryVar);
                hash.set_ilk_info(res.loc, entries.num_ent_strs() as i32);
                entries.set_sort_key_num(entries.num_ent_strs());
                entries.set_num_ent_strs(entries.num_ent_strs() + 1);
                Ok(())
            })?;

            let res = pool.lookup_str_insert(hash, b"entry.max$", StrIlk::BstFn)?;
            hash.set_ty(res.loc, FnClass::IntGlblVar);
            hash.set_ilk_info(res.loc, ENT_STR_SIZE as i32);

            let res = pool.lookup_str_insert(hash, b"global.max$", StrIlk::BstFn)?;
            hash.set_ty(res.loc, FnClass::IntGlblVar);
            hash.set_ilk_info(res.loc, GLOB_STR_SIZE as i32);

            Ok(())
        })
    });
    match res {
        Ok(()) => CResult::Ok,
        Err(BibtexError) => CResult::Error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool() {
        let mut hash = HashData::new();
        let mut new_pool = StringPool::new();
        let res = new_pool
            .lookup_str_insert(&mut hash, b"a cool string", StrIlk::Text)
            .unwrap();
        assert!(!res.exists);
        assert_eq!(
            new_pool.try_get_str(hash.text(res.loc)),
            Ok(b"a cool string" as &[_])
        );

        let res2 = new_pool
            .lookup_str_insert(&mut hash, b"a cool string", StrIlk::Text)
            .unwrap();
        assert!(res2.exists);
        assert_eq!(
            new_pool.try_get_str(hash.text(res2.loc)),
            Ok(b"a cool string" as &[_])
        );

        let res3 = new_pool.lookup_str(&hash, b"a cool string", StrIlk::Text);
        assert!(res3.exists);
        assert_eq!(
            new_pool.try_get_str(hash.text(res3.loc)),
            Ok(b"a cool string" as &[_])
        );

        let res4 = new_pool.lookup_str(&hash, b"a bad string", StrIlk::Text);
        assert!(!res4.exists);
        assert_eq!(
            new_pool.try_get_str(hash.text(res4.loc)),
            Err(LookupErr::DoesntExist)
        );
    }
}
