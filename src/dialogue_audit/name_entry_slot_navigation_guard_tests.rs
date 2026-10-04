use psx_r3000a::{Instruction, Register, verify_placed_program};

use super::name_entry_slot_navigation_guard::{
    ROUTINE_BYTE_COUNT, build_name_entry_slot_navigation_guard_program, next_slot_entry_address,
    previous_slot_entry_address,
};

const OBJECT_ADDRESS: u32 = 0x0000_1000;
const RETURN_ADDRESS: u32 = 0x00de_ad00;
const PLAY_SOUND_ADDRESS: u32 = 0x8018_2c08;
const FIELD_INDEX_OFFSET: usize = 15;
const SLOT_INDEX_OFFSET: usize = 10;
const FAMILY_NAME_RECORD_OFFSET: usize = 0x12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StopReason {
    Returned,
    PlayedMoveSound,
}

struct NameSlotMachine<'a> {
    instructions: &'a [Instruction],
    registers: [u32; 32],
    object: [u8; 0x50],
    pending_load: Option<(Register, u32)>,
}

impl<'a> NameSlotMachine<'a> {
    fn new(instructions: &'a [Instruction], field_index: u8, slot_index: u8) -> Self {
        let mut machine = Self {
            instructions,
            registers: [0; 32],
            object: [0; 0x50],
            pending_load: None,
        };
        machine.write_register(Register::A0, OBJECT_ADDRESS);
        machine.write_register(Register::RA, RETURN_ADDRESS);
        machine.object[FIELD_INDEX_OFFSET] = field_index;
        machine.object[SLOT_INDEX_OFFSET] = slot_index;
        machine
    }

    fn set_slot(&mut self, field_index: usize, slot_index: usize, value: u16) {
        let offset = FAMILY_NAME_RECORD_OFFSET + field_index * 16 + slot_index * 2;
        self.object[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn slot_index(&self) -> u8 {
        self.object[SLOT_INDEX_OFFSET]
    }

    fn register(&self, register: Register) -> u32 {
        self.registers[usize::from(register.index())]
    }

    fn write_register(&mut self, register: Register, value: u32) {
        if register != Register::ZERO {
            self.registers[usize::from(register.index())] = value;
        }
    }

    fn run(&mut self, entry: u32) -> StopReason {
        let mut pc = entry;
        for _ in 0..128 {
            let instruction = self.instruction(pc).clone();
            let destination = match instruction {
                Instruction::Beq { rs, rt, target } => {
                    let taken = self.register(rs) == self.register(rt);
                    self.finish_instruction(None, None);
                    Some(if taken { target } else { pc + 8 })
                }
                Instruction::J { target } => {
                    self.finish_instruction(None, None);
                    Some(target)
                }
                Instruction::Jal { target } => {
                    self.finish_instruction(Some((Register::RA, pc + 8)), None);
                    Some(target)
                }
                Instruction::Jr { rs } => {
                    let target = self.register(rs);
                    self.finish_instruction(None, None);
                    Some(target)
                }
                instruction => {
                    self.execute_non_control(instruction);
                    pc += 4;
                    None
                }
            };
            let Some(destination) = destination else {
                continue;
            };

            self.execute_non_control(self.instruction(pc + 4).clone());
            if destination == PLAY_SOUND_ADDRESS {
                assert_eq!(self.register(Register::A0), 0x91);
                assert_eq!(self.register(Register::RA), RETURN_ADDRESS);
                return StopReason::PlayedMoveSound;
            }
            if destination == RETURN_ADDRESS {
                return StopReason::Returned;
            }
            pc = destination;
        }
        panic!("name-slot program did not return");
    }

    fn instruction(&self, address: u32) -> &Instruction {
        let offset = address
            .checked_sub(previous_slot_entry_address())
            .expect("program jumped before the slot-navigation routine");
        assert_eq!(offset % 4, 0);
        &self.instructions[usize::try_from(offset / 4).unwrap()]
    }

    fn execute_non_control(&mut self, instruction: Instruction) {
        let mut write = None;
        let mut next_load = None;
        match instruction {
            Instruction::Addu { rd, rs, rt } => {
                write = Some((rd, self.register(rs).wrapping_add(self.register(rt))));
            }
            Instruction::Addiu { rt, rs, immediate } => {
                write = Some((
                    rt,
                    self.register(rs).wrapping_add_signed(i32::from(immediate)),
                ));
            }
            Instruction::Andi { rt, rs, immediate } => {
                write = Some((rt, self.register(rs) & u32::from(immediate)));
            }
            Instruction::Ori { rt, rs, immediate } => {
                write = Some((rt, self.register(rs) | u32::from(immediate)));
            }
            Instruction::Xori { rt, rs, immediate } => {
                write = Some((rt, self.register(rs) ^ u32::from(immediate)));
            }
            Instruction::Sltiu { rt, rs, immediate } => {
                let immediate = u32::from_ne_bytes(i32::from(immediate).to_ne_bytes());
                write = Some((rt, u32::from(self.register(rs) < immediate)));
            }
            Instruction::Sltu { rd, rs, rt } => {
                write = Some((rd, u32::from(self.register(rs) < self.register(rt))));
            }
            Instruction::Sll { rd, rt, shift } => {
                write = Some((rd, self.register(rt) << shift));
            }
            Instruction::Lbu { rt, base, offset } => {
                let address = self.effective_address(base, offset);
                next_load = Some((rt, u32::from(self.read_byte(address))));
            }
            Instruction::Lhu { rt, base, offset } => {
                let address = self.effective_address(base, offset);
                let low = self.read_byte(address);
                let high = self.read_byte(address + 1);
                next_load = Some((rt, u32::from(u16::from_le_bytes([low, high]))));
            }
            Instruction::Sb { rt, base, offset } => {
                let address = self.effective_address(base, offset);
                self.write_byte(address, self.register(rt) as u8);
            }
            other => panic!("unsupported test instruction: {other:?}"),
        }
        self.finish_instruction(write, next_load);
    }

    fn finish_instruction(
        &mut self,
        write: Option<(Register, u32)>,
        next_load: Option<(Register, u32)>,
    ) {
        let completed_load = self.pending_load.take();
        if let Some((register, value)) = write {
            self.write_register(register, value);
        }
        if let Some((register, value)) = completed_load {
            self.write_register(register, value);
        }
        self.pending_load = next_load;
        self.registers[usize::from(Register::ZERO.index())] = 0;
    }

    fn effective_address(&self, base: Register, offset: i16) -> u32 {
        self.register(base).wrapping_add_signed(i32::from(offset))
    }

    fn read_byte(&self, address: u32) -> u8 {
        self.object[self.object_offset(address)]
    }

    fn write_byte(&mut self, address: u32, value: u8) {
        let offset = self.object_offset(address);
        self.object[offset] = value;
    }

    fn object_offset(&self, address: u32) -> usize {
        usize::try_from(
            address
                .checked_sub(OBJECT_ADDRESS)
                .expect("test access preceded the name-entry object"),
        )
        .unwrap()
    }
}

#[test]
fn incomplete_current_slot_cannot_be_left_with_next_or_previous() {
    let program = build_name_entry_slot_navigation_guard_program().unwrap();

    let mut next = NameSlotMachine::new(&program.instructions, 0, 0);
    next.set_slot(0, 0, 0xc000);
    assert_eq!(next.run(next_slot_entry_address()), StopReason::Returned);
    assert_eq!(next.slot_index(), 0);

    let mut previous = NameSlotMachine::new(&program.instructions, 0, 1);
    previous.set_slot(0, 1, 0xc001);
    assert_eq!(
        previous.run(previous_slot_entry_address()),
        StopReason::Returned
    );
    assert_eq!(previous.slot_index(), 1);
}

#[test]
fn valid_slots_move_and_preserve_field_limits() {
    let program = build_name_entry_slot_navigation_guard_program().unwrap();

    let mut previous_limit = NameSlotMachine::new(&program.instructions, 0, 0);
    previous_limit.set_slot(0, 0, 0x8001);
    assert_eq!(
        previous_limit.run(previous_slot_entry_address()),
        StopReason::Returned
    );
    assert_eq!(previous_limit.slot_index(), 0);

    let mut previous = NameSlotMachine::new(&program.instructions, 0, 1);
    previous.set_slot(0, 1, 0x8001);
    assert_eq!(
        previous.run(previous_slot_entry_address()),
        StopReason::PlayedMoveSound
    );
    assert_eq!(previous.slot_index(), 0);

    let mut next = NameSlotMachine::new(&program.instructions, 0, 1);
    next.set_slot(0, 1, 0x8001);
    assert_eq!(
        next.run(next_slot_entry_address()),
        StopReason::PlayedMoveSound
    );
    assert_eq!(next.slot_index(), 2);

    let mut family_limit = NameSlotMachine::new(&program.instructions, 0, 5);
    family_limit.set_slot(0, 5, 0x8001);
    assert_eq!(
        family_limit.run(next_slot_entry_address()),
        StopReason::Returned
    );
    assert_eq!(family_limit.slot_index(), 5);

    let mut given_limit = NameSlotMachine::new(&program.instructions, 1, 5);
    given_limit.set_slot(1, 5, 0x8001);
    assert_eq!(
        given_limit.run(next_slot_entry_address()),
        StopReason::Returned
    );
    assert_eq!(given_limit.slot_index(), 5);

    let mut nickname_limit = NameSlotMachine::new(&program.instructions, 2, 3);
    nickname_limit.set_slot(2, 3, 0x8001);
    assert_eq!(
        nickname_limit.run(next_slot_entry_address()),
        StopReason::Returned
    );
    assert_eq!(nickname_limit.slot_index(), 3);
}

#[test]
fn slot_navigation_guard_is_a_complete_typed_replacement() {
    let program = build_name_entry_slot_navigation_guard_program().unwrap();
    let decoded = verify_placed_program(&program.bytes, previous_slot_entry_address()).unwrap();

    assert_eq!(program.bytes.len(), ROUTINE_BYTE_COUNT);
    assert_eq!(decoded, program.instructions);
    assert_eq!(
        decoded[usize::try_from((next_slot_entry_address() - previous_slot_entry_address()) / 4)
            .unwrap()],
        Instruction::Addu {
            rd: Register::T9,
            rs: Register::RA,
            rt: Register::ZERO,
        }
    );
}
