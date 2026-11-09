# Approved GLOBAL bootstrap model policy, before expected images

Root explicitly confirms the qualified reference frontier-seed policy: all old
in-file FREE holes at or below the allocated frontier become USED. Their entire
page bodies remain byte-exact. All old owned, referenced and protected storage
must already be allocated, and stays allocated. This is an explicit exception
to a blanket allocation-bit preservation statement, not permission to overwrite
hole bytes or borrow their pages for append-only allocation.

The four immutable source gates observe EOF 512, canonical Global reference 256,
69-byte INLINE/base 0, no existing bitmap and the following old FREE sets:

- Both V2016 Memo modes: 83.
- Both V2019 Memo modes: 65, 96, 98, 99, 100, 101, 102, 104, 105.

The earlier frozen SOURCE9 protocol and source-review draft used overly broad
language about preserving every allocation bit. Those files stay unchanged as
history. This receipt records root's actual approved policy before any expected
image, public AFTER or candidate is constructed. Source/helper/request bytes do
not change. This is not tuning an image to an observed public result.

The first body is reserved at 512, then the new GLOBAL bitmap at 513. Rewriting
only the uniquely owned original 69-byte global row switches it to a reference
row pointing at 513; the usage-page header, directory, opaque adjacent row and
all other old bytes retain their protection. The new bitmap's physical span is
32736 bits. Its qualified initial FREE fill is byte range [4,4092), while the
final 4 bytes remain zero. This is a pinned reference-writer quirk, not a claim
that Microsoft reserves those bytes. Initial frontier pages 0..512 are USED,
then the self-allocated bitmap 513 is also marked USED. Subsequent typed EOF
reservations clear their own bits on the same cursor.

Single/chain requests resolve every LVAL descriptor before DATA. Column-owned
and candidate rows independently rebase, with the global bitmap excluded from
all body/hint ownership. The 32-row inline batch allocates no LVAL: 21 rows fit
the old DATA page; DATA 512 then GLOBAL 513 precede the remaining 11 rows. Its
ordinary-owned bitmap receives only DATA 512, and ordinary hints rebase to 512.
Both physical index leaves fit; table/primary counts increase by 32 but the
ordinary secondary historical count increases only for the new Group 10 family.

Expected images may import the maintained independent record/DATA/index and
column-map model implementations. They may not import producer/audit code,
staged plans, any Java AFTER bytes/getters, or a new compiled artifact. Every
source raw gate completes before expected image one. The model must independently
decode its resulting maps/chains/index streams, prove exact typed page purposes,
preserve every old hole/live/orphan/Padding/Sentinel body, and retain the nominal
span and exact high-word initialization separately.

This finite engineering policy follows the pinned UsageMap/ByteUtil source and
root's approved literal controls. It does not claim Microsoft Access conformance,
normal save admission or general support for unknown global map shapes.
