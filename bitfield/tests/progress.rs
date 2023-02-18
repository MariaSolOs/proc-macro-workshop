mod playground;

#[test]
fn tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/01-specifier-types.rs");
    t.pass("tests/02-storage.rs");
    t.pass("tests/03-accessors.rs");
    //t.compile_fail("tests/04-multiple-of-8bits.rs");
    //t.pass("tests/05-accessor-signatures.rs");
    //t.pass("tests/06-enums.rs");
    //t.pass("tests/07-optional-discriminant.rs");
    //t.compile_fail("tests/08-non-power-of-two.rs");
    //t.compile_fail("tests/09-variant-out-of-range.rs");
    //t.pass("tests/10-bits-attribute.rs");
    //t.compile_fail("tests/11-bits-attribute-wrong.rs");
    //t.pass("tests/12-accessors-edge.rs");
}

const BOUND: usize = 3;

fn main() {
    let mut x: [u8; 10] = [0; 10];
    r(&mut x, 1, 2);
    println!("{:?}", x);
}

fn r(x: &mut [u8], value: u64, offset: usize) {
    let value = value.to_be_bytes();
    println!("VALUE {:?}", value);

    let mut i = value.len() - 1;
    for v in &mut x[offset..(offset + BOUND)] {
        *v = value[i];
        i -= 1;
    }
}
