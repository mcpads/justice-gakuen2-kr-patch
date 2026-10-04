use psx_r3000a::{Instruction, decode, encode};

use super::name_consumer_hooks::{
    NAME_CONSUMER_HOOK_ADDRESSES, install_dialogue_name_consumer_hooks,
};

const MGAME_RUNTIME_BASE: u32 = 0x800a_2000;
const MGAME_SIZE: usize = 0x0002_e000;
const ORIGINAL_NAME_CONSUMER_ADDRESS: u32 = 0x800a_f9ac;
const SHARED_NAME_CONSUMER_ADDRESS: u32 = 0x8009_a938;

#[test]
fn direct_and_relationship_name_calls_share_the_tagged_name_consumer() {
    let source = synthetic_mgame();
    let mut patched = source.clone();

    let reports =
        install_dialogue_name_consumer_hooks(&source, &mut patched, SHARED_NAME_CONSUMER_ADDRESS)
            .unwrap();

    assert_eq!(
        reports.each_ref().map(|report| report.address.as_str()),
        ["0x800af750", "0x800af8dc"]
    );
    for (address, report) in NAME_CONSUMER_HOOK_ADDRESSES.into_iter().zip(&reports) {
        assert!(report.source_verified);
        assert!(report.installed);
        assert!(!report.runtime_execution_verified);
        assert_eq!(
            decode(read_word(&patched, runtime_offset(address)), address).unwrap(),
            Instruction::Jal {
                target: SHARED_NAME_CONSUMER_ADDRESS,
            }
        );
    }
}

#[test]
fn changed_name_call_is_rejected_before_either_callsite_is_written() {
    let mut source = synthetic_mgame();
    let changed_offset = runtime_offset(NAME_CONSUMER_HOOK_ADDRESSES[1]);
    source[changed_offset..changed_offset + 4].fill(0);
    let mut patched = source.clone();

    assert!(
        install_dialogue_name_consumer_hooks(&source, &mut patched, SHARED_NAME_CONSUMER_ADDRESS,)
            .is_err()
    );
    assert_eq!(patched, source);
}

fn synthetic_mgame() -> Vec<u8> {
    let mut source = vec![0_u8; MGAME_SIZE];
    let instruction = Instruction::Jal {
        target: ORIGINAL_NAME_CONSUMER_ADDRESS,
    };
    for address in NAME_CONSUMER_HOOK_ADDRESSES {
        let offset = runtime_offset(address);
        source[offset..offset + 4]
            .copy_from_slice(&encode(&instruction, address).unwrap().to_le_bytes());
    }
    source
}

fn runtime_offset(address: u32) -> usize {
    usize::try_from(address - MGAME_RUNTIME_BASE).unwrap()
}

fn read_word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
