use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hickory_proto::op::{Message, MessageType, OpCode, Query};
use hickory_proto::rr::{DNSClass, Name, RecordType};

fn build_query_wire() -> Vec<u8> {
    let mut msg = Message::new();
    msg.set_id(0x1234);
    msg.set_message_type(MessageType::Query);
    msg.set_op_code(OpCode::Query);
    msg.set_recursion_desired(true);
    let mut query = Query::new();
    query.set_name(Name::from_ascii("www.example.com.").unwrap());
    query.set_query_type(RecordType::A);
    query.set_query_class(DNSClass::IN);
    msg.add_query(query);
    msg.to_vec().unwrap()
}

fn bench_message_parse(c: &mut Criterion) {
    let wire = build_query_wire();

    c.bench_function("dns_message_parse", |b| {
        b.iter(|| {
            let msg = Message::from_vec(black_box(&wire)).unwrap();
            black_box(msg);
        })
    });
}

fn bench_message_serialize(c: &mut Criterion) {
    let wire = build_query_wire();
    let msg = Message::from_vec(&wire).unwrap();

    c.bench_function("dns_message_serialize", |b| {
        b.iter(|| {
            let bytes = black_box(&msg).to_vec().unwrap();
            black_box(bytes);
        })
    });
}

fn bench_name_parse(c: &mut Criterion) {
    c.bench_function("dns_name_parse", |b| {
        b.iter(|| {
            let name = Name::from_ascii(black_box("www.example.cern.ch.")).unwrap();
            black_box(name);
        })
    });
}

criterion_group!(
    benches,
    bench_message_parse,
    bench_message_serialize,
    bench_name_parse,
);
criterion_main!(benches);
