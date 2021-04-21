use core::fmt;
use core::iter::Iterator;

use crate::utils::*;

type BE = Endian<u32, Big>;

#[repr(C)]
pub struct DeviceTree {
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

impl fmt::Debug for DeviceTree {
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

impl DeviceTree {
    pub unsafe fn from_address<'a>(base: *const Self) -> &'a Self {
        &*base
    }

    pub fn nodes(&self) -> NodeIterator {
        unsafe {
            NodeIterator {
                struct_base: (self as *const _ as *const u8)
                    .offset(self.dt_struct_offset.native() as isize)
                    as *const BE,
                strings_base: (self as *const _ as *const u8)
                    .offset(self.dt_strings_offset.native() as isize),
                depth: 0,
                search_depth: 0,
                _phantom: &(),
            }
        }
    }

    pub fn root(&self) -> Option<Node> {
        self.nodes().next()
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
    pub name: &'a [u8],
    pub value: &'a [u8],
}

pub struct PropIterator<'a> {
    struct_base: *const BE,
    strings_base: *const u8,
    _phantom: &'a (),
}

impl<'a> Iterator for PropIterator<'a> {
    type Item = Prop<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            loop {
                let val = (*self.struct_base).native();
                self.struct_base = self.struct_base.offset(1);
                match val {
                    0x4 => {}
                    0x1 => return None,
                    0x2 => return None,
                    0x9 => return None,
                    0x3 => {
                        let prop = &*(self.struct_base as *const PropHeader);
                        let val_base = (prop as *const PropHeader).offset(1) as *const u8;

                        let string_base = self.strings_base.offset(prop.nameoff.native() as isize);
                        let mut string_len = 0;
                        let mut cur = string_base;
                        while *cur != 0 {
                            cur = cur.offset(1);
                            string_len += 1;
                        }

                        self.struct_base = {
                            let len = prop.len.native() as usize;
                            let mut ptr = (val_base as usize) + len;
                            ptr = (ptr + 4 - 1) & !3;
                            ptr as *const BE
                        };

                        let fullp = Prop {
                            value: core::slice::from_raw_parts(
                                val_base,
                                prop.len.native() as usize,
                            ),
                            name: core::slice::from_raw_parts(string_base, string_len),
                        };
                        return Some(fullp);
                    }
                    _ => panic!("WTF"),
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Node<'a> {
    strings_base: *const u8,
    base: *const BE,
    pub name: &'a [u8],
    pub depth: usize,
}

impl<'a> Node<'a> {
    pub fn props(&self) -> PropIterator<'a> {
        PropIterator {
            struct_base: self.base,
            strings_base: self.strings_base,
            _phantom: &(),
        }
    }

    pub fn prop_by_name(&self, name: &str) -> Option<Prop> {
        let name = name.as_bytes();
        self.props().find(|prop| prop.name == name)
    }

    pub fn children(&self) -> NodeIterator<'a> {
        NodeIterator {
            struct_base: self.base,
            strings_base: self.strings_base,
            depth: self.depth,
            search_depth: 0,
            _phantom: &(),
        }
    }

    pub fn children_by_prop<F>(&self, name: &'static str, matches: F) -> impl Iterator<Item = Node>
    where
        F: Fn(&Prop) -> bool,
    {
        self.children().filter(move |child| {
            if let Some(prop) = child.prop_by_name(name) {
                matches(&prop)
            } else {
                false
            }
        })
    }

    pub fn child_by_name(&self, name: &str) -> Option<Node> {
        let name = name.as_bytes();
        self.children().find(|node| node.name == name)
    }

    fn child_by_path_helper<'b, I: Iterator<Item = &'b [u8]>>(
        self,
        mut path: I,
    ) -> Option<Node<'a>> {
        path.next()
            .and_then(|cur| {
                self.children()
                    .find(|node| node.name == cur)
                    .and_then(|next| next.child_by_path_helper(path))
            })
            .or(Some(self))
    }

    pub fn child_by_path<B: 'a + AsRef<[u8]>>(self, name: B) -> Option<Node<'a>> {
        let mut path = name.as_ref().split(|c| *c == b'/');
        if name.as_ref().first() == Some(&b'/') {
            path.next();
        }
        self.child_by_path_helper(path)
    }
}

pub struct NodeIterator<'a> {
    struct_base: *const BE,
    strings_base: *const u8,
    depth: usize,
    search_depth: usize,
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
                        //let base = self.struct_base;
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
                        let name = core::slice::from_raw_parts(name_base, name_len);
                        self.search_depth += 1;
                        if self.search_depth == 1 {
                            let depth = self.depth + 1;
                            return Some(Node {
                                base: self.struct_base,
                                name,
                                strings_base: self.strings_base,
                                depth,
                            });
                        }
                    }
                    0x2 => {
                        /* End node */
                        if self.search_depth == 0 {
                            return None;
                        }
                        self.search_depth -= 1;
                    }
                    0x3 => {
                        /* Property */
                        let prop = &*(self.struct_base as *const PropHeader);
                        self.struct_base = {
                            let len = prop.len.native() as usize;
                            let mut ptr = (self.struct_base as usize)
                                + core::mem::size_of::<PropHeader>()
                                + len;
                            ptr = (ptr + 4 - 1) & !3;
                            ptr as *const BE
                        };
                    }
                    0x4 => {} // NOP
                    e => panic!("fdt field {:#x} @{:?}", e, self.struct_base.offset(-1)),
                }
            }
        }
    }
}
