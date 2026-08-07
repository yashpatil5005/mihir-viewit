use std::io::{Cursor, Write};

use viewit_core_types::Format;
use viewit_fmt_archive::parse;

/// Build an in-memory ZIP with a handful of text entries.
fn sample_zip() -> Vec<u8> {
    use zip::write::FileOptions;
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        let options: FileOptions<'_, ()> =
            FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for i in 0..8 {
            zip.start_file(format!("dir/file-{i}.txt"), options)
                .unwrap();
            let payload: String = format!("contents of entry {i}\n").repeat(64);
            zip.write_all(payload.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    buf
}

/// Build an in-memory ustar TAR with a handful of text entries.
fn sample_tar() -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut buf);
        for i in 0..8 {
            let mut header = tar::Header::new_ustar();
            header.set_size((128 * 64) as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(
                    &mut header,
                    format!("dir/file-{i}.txt"),
                    &[b'x'; 128 * 64][..],
                )
                .unwrap();
        }
        builder.finish().unwrap();
    }
    buf
}

fn bench_parse(c: &mut criterion::Criterion, format: Format, name: &str, bytes: &[u8]) {
    let format_name = format!("archive/{name}");
    let mut group = c.benchmark_group(format_name);
    group.sample_size(200);
    group.bench_function("parse", |b| {
        b.iter(|| {
            let doc = parse(criterion::black_box(bytes), format, name).unwrap();
            criterion::black_box(doc)
        })
    });
    group.finish();
}

fn parse_zip(c: &mut criterion::Criterion) {
    bench_parse(c, Format::ArchiveZip, "sample.zip", &sample_zip());
}
fn parse_tar(c: &mut criterion::Criterion) {
    bench_parse(c, Format::ArchiveTar, "sample.tar", &sample_tar());
}

criterion::criterion_group!(benches, parse_zip, parse_tar);
criterion::criterion_main!(benches);
