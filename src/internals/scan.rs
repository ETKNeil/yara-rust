use std::convert::{TryFrom, TryInto};
use std::ffi::{CStr, CString};
use std::os::raw::c_void;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

use crate::internals::*;
use crate::Rule;

#[derive(Debug)]
pub enum CallbackMsg<'r> {
    RuleMatching(Rule<'r>),
    RuleNotMatching,
    ScanFinished,
    ImportModule,
    ModuleImported,
    TooManyMatches,
    UnknownMsg,
}

impl<'r> CallbackMsg<'r> {
    fn try_from_yara(
        context: *mut yara_sys::YR_SCAN_CONTEXT,
        message: i32,
        message_data: *mut c_void,
    ) -> Result<Self,Error> {
        use self::CallbackMsg::*;

        Ok(match message as u32 {
            yara_sys::CALLBACK_MSG_RULE_MATCHING => {
                let rule = unsafe { &*(message_data as *mut yara_sys::YR_RULE) };
                let context = unsafe { &*context };
                RuleMatching(Rule::try_from((context, rule))?)
            }
            yara_sys::CALLBACK_MSG_RULE_NOT_MATCHING => RuleNotMatching,
            yara_sys::CALLBACK_MSG_SCAN_FINISHED => ScanFinished,
            yara_sys::CALLBACK_MSG_IMPORT_MODULE => ImportModule,
            yara_sys::CALLBACK_MSG_MODULE_IMPORTED => ModuleImported,
            yara_sys::CALLBACK_MSG_TOO_MANY_MATCHES => TooManyMatches,
            _ => UnknownMsg,
        })
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CallbackReturn {
    Continue,
    Abort,
    Error,
}

impl CallbackReturn {
    pub fn to_yara(self) -> i32 {
        use self::CallbackReturn::*;

        let res = match self {
            Continue => yara_sys::CALLBACK_CONTINUE,
            Abort => yara_sys::CALLBACK_ABORT,
            Error => yara_sys::CALLBACK_ERROR,
        };
        res as i32
    }
}

pub fn rules_scan_mem<'a>(
    rules: *mut yara_sys::YR_RULES,
    mem: &[u8],
    timeout: i32,
    flags: i32,
    mut callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let (user_data, scan_callback) = get_scan_callback(&mut callback);
    let result = unsafe {
        yara_sys::yr_rules_scan_mem(
            rules,
            mem.as_ptr(),
            mem.len().try_into().unwrap(),
            flags,
            scan_callback,
            user_data,
            timeout,
        )
    };

    yara_sys::Error::from_code(result)
        .map_err(|e| e.into())
        .map(|_| ())
}

/// Scan a buffer with the provided YR_SCANNER and its defined external vars.
///
/// Setting the callback function modifies the Scanner with no locks preventing
/// data races, so it should only be called from a &mut Scanner.
pub fn scanner_scan_mem<'a>(
    scanner: *mut yara_sys::YR_SCANNER,
    mem: &[u8],
    mut callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let (user_data, scan_callback) = get_scan_callback(&mut callback);
    let result = unsafe {
        yara_sys::yr_scanner_set_callback(scanner, scan_callback, user_data);
        yara_sys::yr_scanner_scan_mem(scanner, mem.as_ptr(), mem.len().try_into().unwrap())
    };
    yara_sys::Error::from_code(result)
        .map_err(|e| e.into())
        .map(|_| ())
}

#[cfg(unix)]
pub fn rules_scan_file<'a, F: AsRawFd>(
    rules: *mut yara_sys::YR_RULES,
    file: &F,
    timeout: i32,
    flags: i32,
    mut callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let fd = file.as_raw_fd();
    let (user_data, scan_callback) = get_scan_callback(&mut callback);

    let result =
        unsafe { yara_sys::yr_rules_scan_fd(rules, fd, flags, scan_callback, user_data, timeout) };
    yara_sys::Error::from_code(result)
        .map_err(|e| e.into())
        .map(|_| ())
}

#[cfg(windows)]
pub fn rules_scan_file<'a, F: AsRawHandle>(
    rules: *mut yara_sys::YR_RULES,
    file: &F,
    timeout: i32,
    flags: i32,
    mut callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let handle = file.as_raw_handle();
    let (user_data, scan_callback) = get_scan_callback(&mut callback);

    let result = unsafe {
        yara_sys::yr_rules_scan_fd(rules, handle, flags, scan_callback, user_data, timeout)
    };
    yara_sys::Error::from_code(result)
        .map_err(|e| e.into())
        .map(|_| ())
}

#[cfg(unix)]
/// Scan a file with the provided YR_SCANNER and its defined external vars.
///
/// Setting the callback function modifies the Scanner with no locks preventing
/// data races, so it should only be called from a &mut Scanner.
pub fn scanner_scan_file<'a, F: AsRawFd>(
    scanner: *mut yara_sys::YR_SCANNER,
    file: &F,
    mut callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let fd = file.as_raw_fd();
    let (user_data, scan_callback) = get_scan_callback(&mut callback);

    let result = unsafe {
        yara_sys::yr_scanner_set_callback(scanner, scan_callback, user_data);
        yara_sys::yr_scanner_scan_fd(scanner, fd)
    };
    yara_sys::Error::from_code(result)
        .map_err(|e| e.into())
        .map(|_| ())
}

#[cfg(windows)]
/// Scan a file with the provided YR_SCANNER and its defined external vars.
///
/// Setting the callback function modifies the Scanner with no locks preventing
/// data races, so it should only be called from a &mut Scanner.
pub fn scanner_scan_file<'a, F: AsRawHandle>(
    scanner: *mut yara_sys::YR_SCANNER,
    file: &F,
    mut callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let handle = file.as_raw_handle();
    let (user_data, scan_callback) = get_scan_callback(&mut callback);

    let result = unsafe {
        yara_sys::yr_scanner_set_callback(scanner, scan_callback, user_data);
        yara_sys::yr_scanner_scan_fd(scanner, handle)
    };
    yara_sys::Error::from_code(result)
        .map_err(|e| e.into())
        .map(|_| ())
}

/// Attach a process, pause it, and scan its memory.
pub fn rules_scan_proc<'a>(
    rules: *mut yara_sys::YR_RULES,
    pid: u32,
    timeout: i32,
    flags: i32,
    mut callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let (user_data, scan_callback) = get_scan_callback(&mut callback);
    let result = unsafe {
        yara_sys::yr_rules_scan_proc(rules, pid as i32, flags, scan_callback, user_data, timeout)
    };

    yara_sys::Error::from_code(result)
        .map_err(|e| e.into())
        .map(|_| ())
}

/// Attach a process, pause it, and scan its memory with the provided YR_SCANNER
/// and its defined external vars.
///
/// Setting the callback function modifies the Scanner with no locks preventing
/// data races, so it should only be called from a &mut Scanner.
pub fn scanner_scan_proc<'a>(
    scanner: *mut yara_sys::YR_SCANNER,
    pid: u32,
    mut callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let (user_data, scan_callback) = get_scan_callback(&mut callback);
    let result = unsafe {
        yara_sys::yr_scanner_set_callback(scanner, scan_callback, user_data);
        yara_sys::yr_scanner_scan_proc(scanner, pid as i32)
    };
    yara_sys::Error::from_code(result)
        .map_err(|e| e.into())
        .map(|_| ())
}

pub fn scanner_scan_mem_blocks<'a>(
    scanner: *mut yara_sys::YR_SCANNER,
    iter: impl MemoryBlockIterator,
    callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let iter = WrapperMemoryBlockIterator::new(iter).as_yara();
    scanner_scan_mem_blocks_inner(scanner, iter, callback)
}

pub fn scanner_scan_mem_blocks_sized<'a>(
    scanner: *mut yara_sys::YR_SCANNER,
    iter: impl MemoryBlockIteratorSized,
    callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let iter = WrapperMemoryBlockIterator::new(iter).as_yara_sized();
    scanner_scan_mem_blocks_inner(scanner, iter, callback)
}

fn scanner_scan_mem_blocks_inner<'a>(
    scanner: *mut yara_sys::YR_SCANNER,
    mut iter: yara_sys::YR_MEMORY_BLOCK_ITERATOR,
    mut callback: impl FnMut(CallbackMsg<'a>) -> CallbackReturn,
) -> Result<(), Error> {
    let (user_data, scan_callback) = get_scan_callback(&mut callback);
    let result = unsafe {
        yara_sys::yr_scanner_set_callback(scanner, scan_callback, user_data);
        yara_sys::yr_scanner_scan_mem_blocks(scanner, &mut iter as *mut _)
    };
    yara_sys::Error::from_code(result)
        .map_err(|e| e.into())
        .map(|_| ())
}

pub fn get_scan_callback<'a, F>(closure: &mut F) -> (*mut c_void, yara_sys::YR_CALLBACK_FUNC)
where
    F: FnMut(CallbackMsg<'a>) -> CallbackReturn,
{
    (
        closure as *mut F as *mut c_void,
        Some(scan_callback::<'a, F>),
    )
}

extern "C" fn scan_callback<'a, F>(
    context: *mut yara_sys::YR_SCAN_CONTEXT,
    message: i32,
    message_data: *mut c_void,
    user_data: *mut c_void,
) -> i32
where
    F: FnMut(CallbackMsg<'a>) -> CallbackReturn,
{
    let message = CallbackMsg::try_from_yara(context, message, message_data).unwrap_or(CallbackMsg::UnknownMsg);
    let callback = unsafe { &mut *(user_data as *mut F) };
    callback(message).to_yara()
}

/// Setting the flags modifies the Scanner with no locks preventing data races,
/// so it should only be called from a &mut Scanner.
pub fn scanner_set_flags(scanner: *mut yara_sys::YR_SCANNER, flags: i32) {
    unsafe {
        yara_sys::yr_scanner_set_flags(scanner, flags);
    }
}

/// Setting the timeout modifies the Scanner with no locks preventing data races,
/// so it should only be called from a &mut Scanner.
pub fn scanner_set_timeout(scanner: *mut yara_sys::YR_SCANNER, seconds: i32) {
    unsafe {
        yara_sys::yr_scanner_set_timeout(scanner, seconds);
    }
}

pub fn scanner_define_integer_variable(
    scanner: *mut yara_sys::YR_SCANNER,
    identifier: &str,
    value: i64,
) -> Result<(), Error> {
    let identifier = CString::new(identifier).unwrap();
    let result = unsafe {
        yara_sys::yr_scanner_define_integer_variable(scanner, identifier.as_ptr(), value)
    };
    yara_sys::Error::from_code(result).map_err(Into::into)
}

pub fn scanner_define_boolean_variable(
    scanner: *mut yara_sys::YR_SCANNER,
    identifier: &str,
    value: bool,
) -> Result<(), Error> {
    let identifier = CString::new(identifier).unwrap();
    let value = if value { 1 } else { 0 };
    let result = unsafe {
        yara_sys::yr_scanner_define_boolean_variable(scanner, identifier.as_ptr(), value)
    };
    yara_sys::Error::from_code(result).map_err(Into::into)
}

pub fn scanner_define_float_variable(
    scanner: *mut yara_sys::YR_SCANNER,
    identifier: &str,
    value: f64,
) -> Result<(), Error> {
    let identifier = CString::new(identifier).unwrap();
    let result =
        unsafe { yara_sys::yr_scanner_define_float_variable(scanner, identifier.as_ptr(), value) };
    yara_sys::Error::from_code(result).map_err(Into::into)
}

pub fn scanner_define_str_variable(
    scanner: *mut yara_sys::YR_SCANNER,
    identifier: &str,
    value: &str,
) -> Result<(), Error> {
    let identifier = CString::new(identifier).unwrap();
    let value = CString::new(value).unwrap();
    let result = unsafe {
        yara_sys::yr_scanner_define_string_variable(scanner, identifier.as_ptr(), value.as_ptr())
    };
    yara_sys::Error::from_code(result).map_err(Into::into)
}

pub fn scanner_define_cstr_variable(
    scanner: *mut yara_sys::YR_SCANNER,
    identifier: &str,
    value: &CStr,
) -> Result<(), Error> {
    let identifier = CString::new(identifier).unwrap();
    let result = unsafe {
        yara_sys::yr_scanner_define_string_variable(scanner, identifier.as_ptr(), value.as_ptr())
    };
    yara_sys::Error::from_code(result).map_err(Into::into)
}
