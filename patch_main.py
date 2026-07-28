import sys

with open('kernel/src/main.rs', 'r') as f:
    content = f.read()

content = content.replace(
    "PowerReset,\n}",
    "PowerReset,\n    Heterexec,\n}"
)

test_code = """
            SelfTest::Heterexec => {
                kprintln!("HXNU: running kernel self-test = heterexec bridge");
                unsafe {
                    crate::hsched::HSCHED.init();
                    let ptr = crate::hps_bridge::hps_get_shared_buffer();
                    kprintln!("HXNU: heterexec hook shared_buffer ptr={:p}", ptr);
                    
                    let id1 = crate::hsched::HSCHED.submit_workload(crate::hsched::WorkloadType::CpuAvx512).unwrap();
                    let id2 = crate::hsched::HSCHED.submit_workload(crate::hsched::WorkloadType::GpuCompute).unwrap();
                    
                    kprintln!("HXNU: submitted workloads id1={} id2={}", id1, id2);
                    
                    crate::hsched::HSCHED.dispatch_pending();
                    
                    kprintln!("HXNU: dispatched workloads");
                    kprintln!("HXNU: Heterexec bridge self-test PASSED");
                }
            }
"""

content = content.replace(
    "SelfTest::PowerReset => {",
    test_code.strip() + "\n            SelfTest::PowerReset => {"
)

content = content.replace(
    "Some(SelfTest::PowerReset)",
    "Some(SelfTest::PowerReset)\n    } else if cfg!(feature = \"heterexec-self-test\") {\n        Some(SelfTest::Heterexec)"
)

with open('kernel/src/main.rs', 'w') as f:
    f.write(content)
