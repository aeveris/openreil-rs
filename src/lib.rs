//! The `libopenreil` crate aims to provide an okay wrapper around openreil-sys.

extern crate openreil_sys;
extern crate libc;

use openreil_sys::root::{reil_addr_t, reil_inst_t, reil_t, reil_arch_t, reil_inst_print,
                            reil_inst_handler_t, reil_init, reil_close, reil_translate,
                            reil_translate_insn};
pub use openreil_sys::root::{reil_op_t, reil_type_t, reil_size_t, reil_arg_t, reil_raw_t};

use std::mem;
use std::ops::Drop;
use std::marker;


/// Specifies the architecture of the binary code.
///
/// As of now, supported architectures are `x86` and `ARM`.
#[derive(Clone, Copy, Debug)]
pub enum ReilArch {
    X86,
    ARM,
}

/// Callback handler type to further process the resulting REIL instructions.
/// Is a type alias for a C function.
///
/// Please take care to assert that neither argument is NULL.
pub type ReilInstHandler<T> = extern "C" fn(*mut ReilRawInst, *mut T) -> i32;

/// A raw REIL instruction, that is a simple autogenerated wrapper for the original C type.
pub type ReilRawInst = reil_inst_t;

/// A disassembler object.
///
/// The `Reil` type provides a simple interface to disassemble and translate single or multiple instructions
/// to the OpenREIL intermediate language.
///
/// A `handler` callback function can be provided during construction to further process the resulting REIL instructions.
pub struct Reil<'a, T: 'a> {
    reil_handle: reil_t,
    _marker: marker::PhantomData<&'a mut T>,
}

impl<'a, T: 'a> Reil<'a, T> {
    /// Construct a new disassembler object
    /// The handler function can be used to process the resulting REIL instructions
    /// The `context` gets handed to the callback function
    pub fn new(
        arch: ReilArch,
        handler: Option<ReilInstHandler<T>>,
        context: &'a mut T,
    ) -> Option<Self> {
        let arch = match arch {
            ReilArch::X86 => reil_arch_t::ARCH_X86,
            ReilArch::ARM => reil_arch_t::ARCH_ARM,
        };
        let handler: reil_inst_handler_t = unsafe { mem::transmute(handler) };

        let c_ptr = context as *mut _;

        let reil = unsafe { reil_init(arch, handler, c_ptr as *mut libc::c_void) };

        if reil.is_null() {
            return None;
        }

        let new_reil = Reil {
            reil_handle: reil,
            _marker: marker::PhantomData,
        };

        Some(new_reil)
    }

    /// Translate the binary data given in `data` to REIL instructions,
    /// `start_address` designates the starting address the decoded instructions get assigned.
    pub fn translate(&mut self, data: &mut [u8], start_address: u32) {
        unsafe {
            reil_translate(
                self.reil_handle,
                start_address as reil_addr_t,
                data.as_mut_ptr(),
                data.len() as libc::c_int,
            );
        }
    }

    /// Translate a single instruction from the binary data given in `data` and start addressing at the given address.
    pub fn translate_instruction(&mut self, data: &mut [u8], start_address: u32) {
        unsafe {
            reil_translate_insn(
                self.reil_handle,
                start_address as reil_addr_t,
                data.as_mut_ptr(),
                data.len() as libc::c_int,
            );
        }
    }
}

impl<'a, T: 'a> Drop for Reil<'a, T> {
    fn drop(&mut self) {
        unsafe {
            reil_close(self.reil_handle);
        }
    }
}


pub trait ReilInst {
    fn address(&self) -> u64;
    fn reil_offset(&self) -> u8;
    fn raw_address(&self) -> u64;
    fn print(&self);
    fn first_operand(&self) -> Option<reil_arg_t>;
    fn second_operand(&self) -> Option<reil_arg_t>;
    fn third_operand(&self) -> Option<reil_arg_t>;
    fn opcode(&self) -> reil_op_t;
}

impl ReilInst for reil_inst_t {
    fn address(&self) -> u64 {
        let raw_addr = self.raw_address() << 8;
        let reil_offset = self.reil_offset() as u64;
        raw_addr | reil_offset
    }

    fn reil_offset(&self) -> u8 {
        self.inum as u8
    }

    fn raw_address(&self) -> u64 {
        self.raw_info.addr as u64
    }

    fn print(&self) {
        let mut_ptr: *mut reil_inst_t = unsafe { mem::transmute(self) };
        unsafe { reil_inst_print(mut_ptr); }
    }

    fn first_operand(&self) -> Option<reil_arg_t> {
        match self.a.type_ {
            reil_type_t::A_NONE => None,
            _ => Some(self.a)
        }
    }

    fn second_operand(&self) -> Option<reil_arg_t> {
        match self.b.type_ {
            reil_type_t::A_NONE => None,
            _ => Some(self.b)
        }
    }

    fn third_operand(&self) -> Option<reil_arg_t> {
        match self.c.type_ {
            reil_type_t::A_NONE => None,
            _ => Some(self.c)
        }
    }

    fn opcode(&self) -> reil_op_t {
        self.op
    }
}

pub trait ReilArg {
    fn arg_type(&self) -> reil_type_t;
    fn size(&self) -> reil_size_t;
    fn val(&self) -> Option<u64>;
    fn name(&self) -> Option<String>;
}

impl ReilArg for reil_arg_t {
    fn arg_type(&self) -> reil_type_t {
        self.type_
    }

    fn size(&self) -> reil_size_t {
        self.size
    }

    fn val(&self) -> Option<u64> {
        match self.arg_type() {
            reil_type_t::A_CONST | reil_type_t::A_LOC => Some(self.val as u64),
            _ => None,
        }
    }

    fn name(&self) -> Option<String> {
        if self.arg_type() == reil_type_t::A_NONE {
            return None;
        }
        let chars = self.name.iter()
            .take_while(|&b| b as u8 != 0)
            .map(|&b| b as u8)
            .collect();

        String::from_utf8(chars).ok()
    }
}
