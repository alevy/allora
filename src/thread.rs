use alloc::boxed::Box;
use core::sync::atomic::{AtomicU16, Ordering};

use crate::gic;

#[repr(C)]
struct Thread<T: Sized> {
    main: extern "C" fn(Box<Self>),
    stack: Box<[usize; 1024]>,
    userdata: T,
}

extern "C" {
    fn cpu_on(core: usize, main: *mut core::ffi::c_void);
    fn cpu_off(core: usize);
}

extern "C" fn thread_start(mut conf: Box<Thread<Box<dyn FnMut()>>>) {
    (conf.userdata)()
}

static USED_CPUS: AtomicU16 = AtomicU16::new(!0b110);

pub fn spawn<F: 'static + FnMut()>(mut f: F) {
    // Wait until there is a free CPU in the bit map
    let mut used_cpus = USED_CPUS.load(Ordering::Relaxed);
    let mut next_cpu;
    loop {
        loop {
            if used_cpus != !0 {
                break;
            }
            used_cpus = USED_CPUS.load(Ordering::Relaxed);
        }
        next_cpu = used_cpus.trailing_ones() as usize;
        let new_used_cpus = used_cpus | (used_cpus << next_cpu);
        if let Err(uc) =
            USED_CPUS.compare_exchange(used_cpus, new_used_cpus, Ordering::SeqCst, Ordering::SeqCst)
        {
            used_cpus = uc;
        } else {
            used_cpus = new_used_cpus;
            break;
        }
    }

    let conf = Box::into_raw(Box::new(Thread {
        main: thread_start,
        stack: Box::new([0; 1024]),
        userdata: Box::new(move || {
            gic::init();
            f();
            loop {
                let new_used_cpus = used_cpus & !(used_cpus << next_cpu);
                if let Err(uc) = USED_CPUS.compare_exchange(
                    used_cpus,
                    new_used_cpus,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    used_cpus = uc;
                } else {
                    break;
                }
            }
            unsafe { cpu_off(next_cpu) };
        }),
    }));
    unsafe {
        cpu_on(next_cpu, conf as *mut _);
    }
}
