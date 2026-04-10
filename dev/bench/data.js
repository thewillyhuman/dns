window.BENCHMARK_DATA = {
  "lastUpdate": 1775832460820,
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
          "id": "73c4c09370db10956cda6ccf03ad28e7396bb5c8",
          "message": "Add scaling benchmarks for zones, cache, and reload\n\nNew criterion benchmark suite (`scaling.rs`) with 7 benchmark groups:\n\n- **lookup_by_zone_size** -- query latency with 10/100/1k/10k records per zone\n  (exact hit and NXDOMAIN)\n- **lookup_by_zone_count** -- query latency with 1/10/100/1k/10k zones loaded\n  (hit in last zone and miss with no matching zone)\n- **cache_get_by_population** -- cache lookup with 100/1k/10k/100k/500k entries\n  (hit and miss)\n- **cache_insert_by_population** -- cache insert at various fill levels\n- **reload_by_zone_size** -- parse + build store for zones of various sizes\n- **reload_by_zone_count** -- parse + build store for various zone counts\n- **reload_swap** -- atomic zone swap (swap-only vs parse+swap) with\n  1/100/1k/10k background zones\n\nAlso adds ZoneStore::swap_zone() for swapping a pre-parsed zone without\nfile I/O, and includes the scaling bench in CI.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-04-10T15:48:57+02:00",
          "tree_id": "7157c4341db222126e271d81d8dc18d68f6e71be",
          "url": "https://github.com/thewillyhuman/dns/commit/73c4c09370db10956cda6ccf03ad28e7396bb5c8"
        },
        "date": 1775829662921,
        "tool": "cargo",
        "benches": [
          {
            "name": "cache_insert",
            "value": 1659,
            "range": "± 100",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_hit",
            "value": 559,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_miss",
            "value": 287,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "authoritative_a_lookup",
            "value": 6905,
            "range": "± 155",
            "unit": "ns/iter"
          },
          {
            "name": "nxdomain_lookup",
            "value": 6885,
            "range": "± 18",
            "unit": "ns/iter"
          },
          {
            "name": "wildcard_lookup",
            "value": 11375,
            "range": "± 50",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_parse",
            "value": 222,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_serialize",
            "value": 272,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_name_parse",
            "value": 250,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/10",
            "value": 5700,
            "range": "± 203",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/10",
            "value": 9952,
            "range": "± 24",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/100",
            "value": 13090,
            "range": "± 60",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/100",
            "value": 12543,
            "range": "± 42",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/1000",
            "value": 10562,
            "range": "± 37",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/1000",
            "value": 17159,
            "range": "± 92",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/10000",
            "value": 20444,
            "range": "± 108",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/10000",
            "value": 19637,
            "range": "± 84",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/1",
            "value": 8093,
            "range": "± 33",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/1",
            "value": 2066,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/10",
            "value": 8174,
            "range": "± 33",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/10",
            "value": 2064,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/100",
            "value": 8648,
            "range": "± 24",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/100",
            "value": 2251,
            "range": "± 60",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/1000",
            "value": 8316,
            "range": "± 79",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/1000",
            "value": 2098,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/10000",
            "value": 8261,
            "range": "± 30",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/10000",
            "value": 2066,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/100",
            "value": 667,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/100",
            "value": 304,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/1000",
            "value": 673,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/1000",
            "value": 311,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/10000",
            "value": 682,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/10000",
            "value": 314,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/100000",
            "value": 697,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/100000",
            "value": 467,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/500000",
            "value": 707,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/500000",
            "value": 314,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/100",
            "value": 2438,
            "range": "± 111",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/1000",
            "value": 2910,
            "range": "± 274",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/10000",
            "value": 5461,
            "range": "± 916",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/100000",
            "value": 4314,
            "range": "± 711",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/500000",
            "value": 5247,
            "range": "± 3487",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/10",
            "value": 45372,
            "range": "± 192",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/100",
            "value": 622218,
            "range": "± 2526",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/1000",
            "value": 9437034,
            "range": "± 18386",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/10000",
            "value": 127627140,
            "range": "± 1611070",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/1",
            "value": 104596,
            "range": "± 259",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/10",
            "value": 1070373,
            "range": "± 22882",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/100",
            "value": 10713763,
            "range": "± 434142",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/1000",
            "value": 114320651,
            "range": "± 1840368",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/1",
            "value": 4153,
            "range": "± 41",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/1",
            "value": 102904,
            "range": "± 765",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/100",
            "value": 5459,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/100",
            "value": 104452,
            "range": "± 393",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/1000",
            "value": 21302,
            "range": "± 700",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/1000",
            "value": 122573,
            "range": "± 397",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/10000",
            "value": 206280,
            "range": "± 1874",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/10000",
            "value": 305562,
            "range": "± 2174",
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
          "id": "ef6306011537579c35cc39c8874d52e24870a217",
          "message": "Switch zone records from BTreeMap to HashMap for O(1) lookups\n\nBTreeMap's O(log n) lookups caused performance to degrade with zone size,\nespecially on the NXDOMAIN path (20.7µs at 1M records). HashMap gives\nconstant-time lookups regardless of zone size (~4.3µs exact hit, ~4.8µs\nNXDOMAIN). NSEC chain generation now sorts names internally at zone-load\ntime instead of relying on BTreeMap ordering.\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-04-10T16:25:47+02:00",
          "tree_id": "93039eea1b7d9db3cdb9bc4d579da88439a699e5",
          "url": "https://github.com/thewillyhuman/dns/commit/ef6306011537579c35cc39c8874d52e24870a217"
        },
        "date": 1775831902412,
        "tool": "cargo",
        "benches": [
          {
            "name": "cache_insert",
            "value": 1649,
            "range": "± 133",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_hit",
            "value": 553,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_miss",
            "value": 283,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "authoritative_a_lookup",
            "value": 5330,
            "range": "± 189",
            "unit": "ns/iter"
          },
          {
            "name": "nxdomain_lookup",
            "value": 6012,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "wildcard_lookup",
            "value": 9176,
            "range": "± 383",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_parse",
            "value": 218,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_serialize",
            "value": 269,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_name_parse",
            "value": 248,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/100",
            "value": 6704,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/100",
            "value": 7785,
            "range": "± 134",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/100000",
            "value": 6900,
            "range": "± 68",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/100000",
            "value": 7886,
            "range": "± 39",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/1000000",
            "value": 6809,
            "range": "± 26",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/1000000",
            "value": 7382,
            "range": "± 16",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/1",
            "value": 6657,
            "range": "± 26",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/1",
            "value": 2068,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/1000",
            "value": 6839,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/1000",
            "value": 2078,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/10000",
            "value": 6866,
            "range": "± 107",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/10000",
            "value": 2059,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/100000",
            "value": 6877,
            "range": "± 34",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/100000",
            "value": 2127,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/100",
            "value": 664,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/100",
            "value": 307,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/100000",
            "value": 1137,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/100000",
            "value": 317,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/1000000",
            "value": 1137,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/1000000",
            "value": 462,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/100",
            "value": 1973,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/100000",
            "value": 2012,
            "range": "± 113",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/1000000",
            "value": 2231,
            "range": "± 145",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/100",
            "value": 399593,
            "range": "± 2029",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/100000",
            "value": 847516377,
            "range": "± 2146698",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/1000000",
            "value": 10162454356,
            "range": "± 17975061",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/1",
            "value": 77714,
            "range": "± 135",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/1000",
            "value": 87129722,
            "range": "± 278455",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/10000",
            "value": 870424217,
            "range": "± 2781937",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/100000",
            "value": 8887618087,
            "range": "± 18050304",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/1",
            "value": 3931,
            "range": "± 209",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/1",
            "value": 75805,
            "range": "± 324",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/1000",
            "value": 20741,
            "range": "± 119",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/1000",
            "value": 95365,
            "range": "± 511",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/10000",
            "value": 195818,
            "range": "± 1916",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/10000",
            "value": 270015,
            "range": "± 1306",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/100000",
            "value": 5395176,
            "range": "± 975929",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/100000",
            "value": 5537516,
            "range": "± 874776",
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
          "id": "cfa269dfabf531fde86b1adc7231890d9da4f436",
          "message": "Update README performance section with scaling benchmark results\n\nCo-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>",
          "timestamp": "2026-04-10T16:35:07+02:00",
          "tree_id": "9c1d0222add02da94722f15649e1ecd5eae41b90",
          "url": "https://github.com/thewillyhuman/dns/commit/cfa269dfabf531fde86b1adc7231890d9da4f436"
        },
        "date": 1775832459993,
        "tool": "cargo",
        "benches": [
          {
            "name": "cache_insert",
            "value": 1645,
            "range": "± 97",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_hit",
            "value": 559,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_miss",
            "value": 289,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "authoritative_a_lookup",
            "value": 5334,
            "range": "± 12",
            "unit": "ns/iter"
          },
          {
            "name": "nxdomain_lookup",
            "value": 6080,
            "range": "± 107",
            "unit": "ns/iter"
          },
          {
            "name": "wildcard_lookup",
            "value": 9434,
            "range": "± 203",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_parse",
            "value": 211,
            "range": "± 0",
            "unit": "ns/iter"
          },
          {
            "name": "dns_message_serialize",
            "value": 281,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "dns_name_parse",
            "value": 252,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/100",
            "value": 6332,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/100",
            "value": 7627,
            "range": "± 36",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/100000",
            "value": 6618,
            "range": "± 33",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/100000",
            "value": 7494,
            "range": "± 26",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/exact_hit/1000000",
            "value": 6558,
            "range": "± 22",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_size/nxdomain/1000000",
            "value": 7368,
            "range": "± 53",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/1",
            "value": 6328,
            "range": "± 55",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/1",
            "value": 2112,
            "range": "± 32",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/1000",
            "value": 6520,
            "range": "± 40",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/1000",
            "value": 2060,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/10000",
            "value": 6530,
            "range": "± 26",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/10000",
            "value": 2064,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/hit_last_zone/100000",
            "value": 6915,
            "range": "± 25",
            "unit": "ns/iter"
          },
          {
            "name": "lookup_by_zone_count/miss_no_zone/100000",
            "value": 2111,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/100",
            "value": 662,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/100",
            "value": 310,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/100000",
            "value": 901,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/100000",
            "value": 464,
            "range": "± 1",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/hit/1000000",
            "value": 702,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "cache_get_by_population/miss/1000000",
            "value": 316,
            "range": "± 2",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/100",
            "value": 1995,
            "range": "± 21",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/100000",
            "value": 1926,
            "range": "± 68",
            "unit": "ns/iter"
          },
          {
            "name": "cache_insert_by_population/insert/1000000",
            "value": 2003,
            "range": "± 69",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/100",
            "value": 396217,
            "range": "± 841",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/100000",
            "value": 847693538,
            "range": "± 1759828",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_size/parse_and_build/1000000",
            "value": 10125783650,
            "range": "± 43726108",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/1",
            "value": 76636,
            "range": "± 71",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/1000",
            "value": 83078602,
            "range": "± 1706712",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/10000",
            "value": 865547951,
            "range": "± 8545047",
            "unit": "ns/iter"
          },
          {
            "name": "reload_by_zone_count/parse_and_build_all/100000",
            "value": 8828976425,
            "range": "± 27817751",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/1",
            "value": 3924,
            "range": "± 39",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/1",
            "value": 75924,
            "range": "± 183",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/1000",
            "value": 20887,
            "range": "± 846",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/1000",
            "value": 94810,
            "range": "± 214",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/10000",
            "value": 193151,
            "range": "± 928",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/10000",
            "value": 265441,
            "range": "± 786",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/swap_only/100000",
            "value": 3076871,
            "range": "± 47965",
            "unit": "ns/iter"
          },
          {
            "name": "reload_swap/parse_and_swap/100000",
            "value": 3282441,
            "range": "± 58277",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}