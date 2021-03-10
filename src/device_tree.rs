use core::fmt;

#[derive(Copy, Clone)]
pub struct BE(u32);

impl BE {
    pub fn native(self) -> u32 {
        u32::from_be(self.0)
    }
}

impl fmt::Display for BE {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.native())
    }
}

impl fmt::Debug for BE {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.native())
    }
}

impl fmt::LowerHex for BE {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.native())
    }
}

#[repr(C)]
pub struct Header {
    magic: BE,
    total_size: BE,
    dt_struct_offset: BE,
    dt_strings_offset: BE,
    memory_reserve_map_offset: BE,
    version: BE,
    last_compatible_version: BE,
    boot_cpuid: BE,
    dt_strings_size: BE,
    dt_struct_size: BE,
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Header {\n")?;
        write!(f, "  magic: {:#x}\n", self.magic)?;
        write!(f, "  total_size: {:#x}\n", self.total_size)?;
        write!(f, "  dt_struct_offset: {}\n", self.dt_struct_offset)?;
        write!(f, "  dt_strings_offset: {}\n", self.dt_strings_offset)?;
        write!(
            f,
            "  memory_reserve_map_offset: {}\n",
            self.memory_reserve_map_offset
        )?;
        write!(f, "  version: {}\n", self.version)?;
        write!(
            f,
            "  last_compatible_version: {}\n",
            self.last_compatible_version
        )?;
        write!(f, "  boot_cpuid: {}\n", self.boot_cpuid)?;
        write!(f, "  dt_strings_size: {}\n", self.dt_strings_size)?;
        write!(f, "  dt_struct_size: {}\n", self.dt_struct_size)?;
        f.write_str("}")
    }
}

impl Header {
    pub unsafe fn nodes(&self) -> NodeIterator {
        NodeIterator {
            struct_base: (self as *const _ as *const u8)
                .offset(self.dt_struct_offset.native() as isize)
                as *const BE,
            strings_base: (self as *const _ as *const u8)
                .offset(self.dt_strings_offset.native() as isize),
            depth: 0,
            _phantom: &(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PropHeader {
   len: BE,
   nameoff: BE,
}

#[derive(Debug)]
pub struct Prop<'a> {
    header: &'a PropHeader,
    val: &'a [u8],
    name: &'a str,
}

#[derive(Debug)]
pub struct Node<'a> {
    strings_base: *const u8,
    base: *const BE,
    pub name: &'a str,
    pub depth: usize,
}

impl<'a> Node<'a> {
    pub fn prop(&self, _i: usize) -> Option<Prop> {
        unsafe {
            let mut struct_base = self.base.offset(1);
            loop {
                let val = (*struct_base).native();
                struct_base = struct_base.offset(1);
                if val == 0x3 {
                    break
                } else if val == 0x2 {
                    return None;
                }
            }
            let prop = &*(struct_base as *const PropHeader);
            let val_base = (prop as *const PropHeader).offset(1) as *const u8;

            let string_base = self.strings_base.offset(prop.nameoff.native() as isize);
            let mut string_len = 0;
            let mut cur = string_base;
            while *cur != 0 {
                cur = cur.offset(1);
                string_len += 1;
            }

            let fullp = Prop {
                header: prop,
                val: core::slice::from_raw_parts(val_base, prop.len.native() as usize),
                name: core::str::from_utf8_unchecked(core::slice::from_raw_parts(string_base, string_len)),
            };
            Some(fullp)
        }
    }
}

pub struct NodeIterator<'a> {
    struct_base: *const BE,
    strings_base: *const u8,
    depth: usize,
    _phantom: &'a (),
}

impl<'a> Iterator for NodeIterator<'a> {
    type Item = Node<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            loop {
                let val = (*self.struct_base).native();
                if val == 0x9 {
                    return None;
                }
                self.struct_base = self.struct_base.offset(1);
                match val {
                    0x1 => {
                        /* Begin node */
                        let base = self.struct_base;
                        let name_base = self.struct_base as *const u8;
                        let mut name_base_cur = name_base;
                        let mut name_len = 0;
                        while *name_base_cur != 0 {
                            name_base_cur = name_base_cur.offset(1);
                            name_len += 1;
                        }
                        let mut next_base_i = (name_base_cur as usize) + 1; // one past the null byte
                        next_base_i = (next_base_i + 4 - 1) & !3;
                        self.struct_base = next_base_i as *const BE;
                        let name = core::str::from_utf8_unchecked(core::slice::from_raw_parts(name_base, name_len));
                        let depth = self.depth;
                        self.depth += 1;
                        return Some(Node { base, name, strings_base: self.strings_base, depth });
                    },
                    0x2 => {
                        /* End node */
                        self.depth -= 1;
                    },
                    0x3 => {
                        /* Property */
                        let prop = &*(self.struct_base as *const PropHeader);
                        self.struct_base = {
                            let len = prop.len.native() as usize;
                            let mut ptr = (self.struct_base as usize) + core::mem::size_of::<PropHeader>() + len;
                            ptr = (ptr + 4 - 1) & !3;
                            ptr as *const BE
                        };
                    }
                    0x4 => {}, // NOP
                    e => panic!("fdt field {:#x} @{:?}", e, self.struct_base.offset(-1)),
                }
            }
        }
    }
}
