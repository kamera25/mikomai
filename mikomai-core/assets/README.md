# Shared runtime assets

`network/config_helper.py` reads JSON from stdin and writes one JSON result to
stdout. It locates the vendor templates relative to its own file at
`../templates`; callers can run `python3 mikomai-core/assets/network/config_helper.py`
with `{"action":"validate","config":"..."}` or
`{"action":"convert","target_vendor":"juniper","config":"..."}`.
The converter uses Jinja2. `ciscoconfparse2` is optional; a standard-library
parser handles host name, DNS, NTP, interface, and static-route directives when
it is absent.

`network/nwdiag_wrapper.py` preserves nwdiag's SVG command interface. Call it
with `-T svg -o output.svg input.diag`. The Python environment needs nwdiag and
Pillow installed. The wrapper includes compatibility shims for older nwdiag
and newer Pillow/setuptools versions.
