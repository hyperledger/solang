// SPDX-License-Identifier: Apache-2.0

use crate::build_solidity;
use ink_env::hash::{Blake2x256, CryptoHash};
use ink_primitives::{AccountId, Hash};
use parity_scale_codec::Encode;
use solang::{file_resolver::FileResolver, Target};
use std::ffi::OsStr;

struct PrefixedValue<'a, 'b, T> {
    pub prefix: &'a [u8],
    pub value: &'b T,
}

impl<X: Encode> Encode for PrefixedValue<'_, '_, X> {
    #[inline]
    fn size_hint(&self) -> usize {
        self.prefix.size_hint() + self.value.size_hint()
    }

    #[inline]
    fn encode_to<T: parity_scale_codec::Output + ?Sized>(&self, dest: &mut T) {
        self.prefix.encode_to(dest);
        self.value.encode_to(dest);
    }
}

fn topic_hash(encoded: &[u8]) -> Hash {
    let mut buf = [0; 32];
    if encoded.len() <= 32 {
        buf[..encoded.len()].copy_from_slice(encoded);
    } else {
        <Blake2x256 as CryptoHash>::hash(encoded, &mut buf);
    };
    buf.into()
}

#[test]
fn anonymous() {
    let mut runtime = build_solidity(
        r##"
        contract a {
            event foo(bool b) anonymous;
            function emit_event() public {
                emit foo(true);
            }
        }"##,
    );

    runtime.constructor(0, Vec::new());
    runtime.function("emit_event", Vec::new());

    assert_eq!(runtime.events().len(), 1);
    let event = &runtime.events()[0];
    assert_eq!(event.topics.len(), 0);
    assert_eq!(event.data, (0u8, true).encode());
}

#[test]
fn emit() {
    #[derive(Encode)]
    enum Event {
        Foo(bool, u32, i64),
        Bar(u32, u64, String),
    }

    let mut runtime = build_solidity(
        r#"
        contract a {
            event foo(bool,uint32,int64 indexed i);
            event bar(uint32,uint64,string indexed s);
            function emit_event() public {
                emit foo(true, 102, 1);
                emit bar(0xdeadcafe, 102, "foobar");
            }
        }"#,
    );

    runtime.constructor(0, Vec::new());
    runtime.function("emit_event", Vec::new());

    assert_eq!(runtime.events().len(), 2);
    let event = &runtime.events()[0];
    assert_eq!(event.topics.len(), 2);
    assert_eq!(event.topics[0], topic_hash(b"\0a::foo"));
    let topic = PrefixedValue {
        prefix: b"a::foo::i",
        value: &1i64,
    }
    .encode();
    assert_eq!(event.topics[1], topic_hash(&topic[..]));
    assert_eq!(event.data, Event::Foo(true, 102, 1).encode());

    let event = &runtime.events()[1];
    assert_eq!(event.topics.len(), 2);
    println!("topic hash: {:?}", event.topics[0]);
    println!("topic hash: {:?}", event.topics[0]);
    assert_eq!(event.topics[0], topic_hash(b"\0a::bar"));
    let topic = PrefixedValue {
        prefix: b"a::bar::s",
        value: &String::from("foobar"),
    }
    .encode();
    assert_eq!(event.topics[1], topic_hash(&topic[..]));
    assert_eq!(
        event.data,
        Event::Bar(0xdeadcafe, 102, "foobar".into()).encode()
    );
}

#[test]
fn event_imported() {
    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol";

        contract foo {
            function emit_event() public {
                emit bar(102, true);
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        event bar (uint32 indexed f1, bool x);
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol";

        contract foo {
            function emit_event() public {
                emit baz.bar(102, true);
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        abstract contract baz {
            event bar (uint32 indexed f1, bool x);
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol" as X;

        contract foo {
            function emit_event() public {
                emit X.baz.bar(102, true);
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        abstract contract baz {
            event bar (uint32 indexed f1, bool x);
        }
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());

    let mut cache = FileResolver::default();

    cache.set_file_contents(
        "a.sol",
        r#"
        import "b.sol" as X;

        contract foo {
            function emit_event() public {
                emit X.bar(102, true);
            }
        }
        "#
        .to_string(),
    );

    cache.set_file_contents(
        "b.sol",
        r#"
        event bar (uint32 indexed f1, bool x);
        "#
        .to_string(),
    );

    let ns = solang::parse_and_resolve(OsStr::new("a.sol"), &mut cache, Target::default_polkadot());

    assert!(!ns.diagnostics.any_errors());
}

/// FIXME: Use the exact same event structure once the `Option<T>` type is available
#[test]
fn erc20_ink_example() {
    #[derive(Encode)]
    enum Event {
        Transfer(AccountId, AccountId, u128),
    }

    #[derive(Encode)]
    struct Transfer {
        from: AccountId,
        to: AccountId,
        value: u128,
    }

    let mut runtime = build_solidity(
        r##"
        contract Erc20 {
            event Transfer(
                address indexed from,
                address indexed to,
                uint128 value
            );

            function emit_event(address from, address to, uint128 value) public {
                emit Transfer(from, to, value);
            }
        }"##,
    );
    runtime.constructor(0, Vec::new());
    let from = AccountId::from([1; 32]);
    let to = AccountId::from([2; 32]);
    let value = 10;
    runtime.function("emit_event", Transfer { from, to, value }.encode());

    assert_eq!(runtime.events().len(), 1);
    let event = &runtime.events()[0];
    assert_eq!(event.data, Event::Transfer(from, to, value).encode());

    assert_eq!(event.topics.len(), 3);
    assert_eq!(event.topics[0], topic_hash(b"\0Erc20::Transfer"));

    let expected_topic = PrefixedValue {
        prefix: b"Erc20::Transfer::from",
        value: &from,
    };
    assert_eq!(event.topics[1], topic_hash(&expected_topic.encode()));

    let expected_topic = PrefixedValue {
        prefix: b"Erc20::Transfer::to",
        value: &to,
    };
    assert_eq!(event.topics[2], topic_hash(&expected_topic.encode()));
}

#[test]
fn freestanding() {
    let mut runtime = build_solidity(
        r##"
        event A(bool indexed b);
        function foo() {
            emit A(true);
        }
        contract a {
            function emit_event() public {
                foo();
            }
        }"##,
    );

    runtime.constructor(0, Vec::new());
    runtime.function("emit_event", Vec::new());

    assert_eq!(runtime.events().len(), 1);
    let event = &runtime.events()[0];
    assert_eq!(event.data, (0u8, true).encode());
    assert_eq!(event.topics[0], topic_hash(b"\0a::A"));
    let expected_topic = PrefixedValue {
        prefix: b"a::A::b",
        value: &true,
    };
    assert_eq!(event.topics[1], topic_hash(&expected_topic.encode()));
}

#[test]
fn different_contract() {
    let mut runtime = build_solidity(
        r##"abstract contract A { event X(bool indexed foo); }
        contract B { function emit_event() public { emit A.X(true); } }"##,
    );

    runtime.constructor(0, Vec::new());
    runtime.function("emit_event", Vec::new());

    assert_eq!(runtime.events().len(), 1);
    let event = &runtime.events()[0];
    assert_eq!(event.data, (0u8, true).encode());
    assert_eq!(event.topics[0], topic_hash(b"\0A::X"));
    let expected_topic = PrefixedValue {
        prefix: b"A::X::foo",
        value: &true,
    };
    assert_eq!(event.topics[1], topic_hash(&expected_topic.encode()));
}
