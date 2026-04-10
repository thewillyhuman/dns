window.BENCHMARK_DATA = {
  "lastUpdate": 1775825309345,
  "repoUrl": "https://github.com/thewillyhuman/dns",
  "entries": {
    "DNS Server Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "guillermo.facundo.colunga@cern.ch",
            "name": "Guillermo Facundo Colunga",
            "username": "thewillyhuman"
          },
          "committer": {
            "email": "guillermo.facundo.colunga@cern.ch",
            "name": "Guillermo Facundo Colunga",
            "username": "thewillyhuman"
          },
          "distinct": true,
          "id": "5636b7d8dee4ba9dc829c1695794a380b4f5e07f",
          "message": "Fix clippy warnings and rustfmt formatting for CI\n\n- Remove unused CacheStats import in cache_test.rs\n- Replace or_insert_with(Default::new) with or_default()\n- Remove redundant & on format!() args passed to from_ascii()\n- Rename ServerConfig::from_str to parse_toml to avoid confusion with FromStr trait\n- Derive Default for ServerConfig instead of manual impl\n- Allow clippy::too_many_arguments on sign_rrset (DNSSEC signing params)\n- Replace &[x.clone()] with std::slice::from_ref(&x)\n- Replace len() >= 1 with !is_empty()\n- Remove redundant let server_addr = server_addr rebindings\n- Use is_multiple_of() instead of manual % check\n- Apply rustfmt to all files\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-04-10T14:17:35+02:00",
          "tree_id": "f905503df4922fbe15381d1bcb160e5004c9506e",
          "url": "https://github.com/thewillyhuman/dns/commit/5636b7d8dee4ba9dc829c1695794a380b4f5e07f"
        },
        "date": 1775823993331,
        "tool": "cargo",
        "benches": [
          {
            "name": "cache_insert",
            "value": 1669,
            "range": "± 79",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_hit",
            "value": 585,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_miss",
            "value": 302,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "authoritative_a_lookup",
            "value": 6908,
            "range": "± 71",
            "unit": "ns/iter"
          },
          {
            "name": "nxdomain_lookup",
            "value": 6922,
            "range": "± 28",
            "unit": "ns/iter"
          },
          {
            "name": "wildcard_lookup",
            "value": 11348,
            "range": "± 62",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_parse",
            "value": 233,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_serialize",
            "value": 277,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "dns_name_parse",
            "value": 262,
            "range": "± 1",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "guillermo.facundo.colunga@cern.ch",
            "name": "Guillermo Facundo Colunga",
            "username": "thewillyhuman"
          },
          "committer": {
            "email": "guillermo.facundo.colunga@cern.ch",
            "name": "Guillermo Facundo Colunga",
            "username": "thewillyhuman"
          },
          "distinct": true,
          "id": "fa48af24db1918735402b887b5148491f802c58d",
          "message": "Rename binary from cern-dns to dns and remove CERN branding\n\n- Binary is now `dns` instead of `cern-dns`\n- README title changed to \"DNS\"\n- Updated all docs (installation, operations) to reference `dns` binary\n- Example zone data and test fixtures left unchanged\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-04-10T14:46:05+02:00",
          "tree_id": "a05dcb8712fcbbb0458034782004e16dee38e92c",
          "url": "https://github.com/thewillyhuman/dns/commit/fa48af24db1918735402b887b5148491f802c58d"
        },
        "date": 1775825308940,
        "tool": "cargo",
        "benches": [
          {
            "name": "cache_insert",
            "value": 1761,
            "range": "± 95",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_hit",
            "value": 491,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_miss",
            "value": 245,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "authoritative_a_lookup",
            "value": 5891,
            "range": "± 54",
            "unit": "ns/iter"
          },
          {
            "name": "nxdomain_lookup",
            "value": 6060,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "wildcard_lookup",
            "value": 9883,
            "range": "± 24",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_parse",
            "value": 182,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_serialize",
            "value": 244,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "dns_name_parse",
            "value": 211,
            "range": "± 1",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}