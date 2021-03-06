use arch::memory::MemoryController;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::structures::idt::{Idt, ExceptionStackFrame};
use spin::Once;

pub mod gdt;
pub mod exceptions;
pub mod irq;
pub mod utils;

pub use self::utils::*;

const DOUBLE_FAULT_IST_INDEX: usize = 0;

lazy_static! {
    static ref IDT: Idt = {
        let mut idt = Idt::new();

        println!("[ interrupts ] Installing exception handlers.");
        idt.divide_by_zero.set_handler_fn(exceptions::divide_by_zero_handler);
        idt.debug.set_handler_fn(exceptions::debug_handler);
        idt.non_maskable_interrupt.set_handler_fn(exceptions::nmi_handler);
        idt.breakpoint.set_handler_fn(exceptions::breakpoint_handler);
        idt.overflow.set_handler_fn(exceptions::overflow_handler);
        idt.bound_range_exceeded.set_handler_fn(exceptions::bound_range_handler);
        idt.invalid_opcode.set_handler_fn(exceptions::invalid_opcode_handler);
        idt.device_not_available.set_handler_fn(exceptions::device_not_available_handler);
        unsafe {
            idt.double_fault.set_handler_fn(exceptions::double_fault_handler)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX as u16);
        }
        idt.invalid_tss.set_handler_fn(exceptions::invalid_tss_handler);
        idt.segment_not_present.set_handler_fn(exceptions::seg_not_present_handler);
        idt.stack_segment_fault.set_handler_fn(exceptions::stack_seg_fault_handler);
        idt.general_protection_fault.set_handler_fn(exceptions::gpf_handler);
        idt.page_fault.set_handler_fn(exceptions::page_fault_handler);
        idt.x87_floating_point.set_handler_fn(exceptions::x87_fp_exception_handler);
        idt.alignment_check.set_handler_fn(exceptions::alignment_check_handler);
        idt.machine_check.set_handler_fn(exceptions::machine_check_handler);
        idt.simd_floating_point.set_handler_fn(exceptions::simd_fp_exception_handler);

        println!("[ interrupts ] Installing IRQs.");
        idt.interrupts[0].set_handler_fn(irq::timer_handler);
        // idt.interrupts[1].set_handler_fn(irq::keyboard_handler);
        
        idt.interrupts[0x30 - 0x20].set_handler_fn(irq::timer_handler);
        // idt.interrupts[17].set_handler_fn(irq::keyboard_handler);
        
        // APIC NMI.
        for vec in (0x90-0x20)..(0x97-0x20) {
            idt.interrupts[vec].set_handler_fn(apic_nmi_handler);
        }
        idt.interrupts[0xff - 0x20].set_handler_fn(spurious_interrupt_handler);

        idt
    };
}

static TSS: Once<TaskStateSegment> = Once::new();
static GDT: Once<gdt::Gdt> = Once::new();

/// Loads an IDT, GDT and TSS and reloads code segment registers.
pub fn init(memory_controller: &mut MemoryController) {
    use x86_64::structures::gdt::SegmentSelector;
    use x86_64::instructions::segmentation::set_cs;
    use x86_64::instructions::tables::load_tss;
    use x86_64::VirtualAddress;

    let double_fault_stack = memory_controller
        .alloc_stack(1)
        .expect("could not allocate double fault stack");

    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX] =
            VirtualAddress(double_fault_stack.top());
        //TODO allocate privilege stacks.
        tss
    });

    let mut code_selector = SegmentSelector(0);
    let mut tss_selector = SegmentSelector(0);
    let gdt = GDT.call_once(|| {
        let mut gdt = gdt::Gdt::new();
        println!("[ tables ] Loading GDT entries.");
        code_selector = gdt.add_entry(gdt::Descriptor::kernel_code_segment());
        tss_selector = gdt.add_entry(gdt::Descriptor::tss_segment(&tss));
        gdt
    });

    // Load a new GDT in the CPU.
    gdt.load();
    println!("[ tables ] Successfully loaded GDT.");

    unsafe {
        // reload code segment register.
        println!("[ tables ] Reloading CS.");
        set_cs(code_selector);
        // load TSS
        println!("[ tables ] Loading TSS.");
        load_tss(tss_selector);
    }

    // Load the IDT
    IDT.load();
    println!("[ tables ] Successfully loaded IDT.")
}

pub extern "x86-interrupt" fn apic_nmi_handler(stack_frame: &mut ExceptionStackFrame) {
    println!("NON-MASKABLE APIC INTERRUPT!");
    loop {}
}

pub extern "x86-interrupt" fn spurious_interrupt_handler(stack_frame: &mut ExceptionStackFrame) {
    println!("SPURIOUS INTERRUPT!");
}
