# TIFF ICC selected-page/error boundary controls

Three additional ordinary tests qualify the native ICC slice extractor's routing and refusal boundaries. No production behavior changed in this follow-up.

- Two linked IFDs hold distinct inline profile markers (`one`/`two`). Classic TIFF/BigTIFF × II/MM (4variants) run both page selections through the real selected_pointer header rewrite, observe the proper profile, restore the header and verify the entire source stays identical. Out-of-range page selection also preserves source.
- An empty ICC on the selected second page is passed through the actual outer raster decoder. It returns typed InvalidTiffDirectory before pixel decoding/admission, restores the borrowed source header byte-for-byte, leaves zero managed use/peak, and the restored first-page profile remains readable. All4variants pass.
- Missing profile returns None; empty, duplicate, invalid external extent and excessive4097-entry tables return explicit errors under all4variants. Earlier tests retain unsupported type/truncated table/data coverage.
- An already-cancelled token precedes invalid header parsing and leaves managed use/peak at0.

Full public decoder library: **208 passed /5 ignored /zero failures**. New test names and raw output are in `public.log`; current source hashes are retained. These authored metadata fixtures are not complete producer image files or proof of ICC color interpretation, cross-page GPU rendering or complete TIFF interoperability. The prior pinned oriented TIFF/public large-profile gates provide separate pixel/admission evidence.
