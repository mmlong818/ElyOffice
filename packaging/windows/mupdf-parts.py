"""Usage: mupdf-parts.py <mupdf-src> parts|fonts; what MSBuild compiled."""
import re
import sys

src, what = sys.argv[1], sys.argv[2]
projects = f"{src}/platform/win32"
# The libraries build-mupdf.sh links into the worker.
built = ["libthirdparty", "libextract", "libharfbuzz", "libleptonica", "libmubarcode",
         "libtesseract", "libmupdf", "libpkcs7"]
compiled = re.compile(r'<ClCompile Include="\.\.\\\.\.\\thirdparty\\([^\\"]+)\\')
embedded = re.compile(r'Include="\.\.\\\.\.\\resources\\fonts\\([^\\"]+)\\')
found = set()
if what == "parts":
    for name in built:
        with open(f"{projects}/{name}.vcxproj", encoding="utf-8") as project:
            found |= set(compiled.findall(project.read()))
elif what == "fonts":
    with open(f"{projects}/libresources.vcxproj", encoding="utf-8") as project:
        found = set(embedded.findall(project.read()))
else:
    sys.exit(f"mupdf-parts.py: parts or fonts, not {what}")
if not found:
    sys.exit(f"mupdf-parts.py: no {what} in {projects}")
# Bash reads these names; Windows would end them CRLF.
sys.stdout.reconfigure(newline="\n")
print("\n".join(sorted(found)))
