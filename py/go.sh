cd py
export PY=c:/python311
maturin build -i $PY/python.exe \
&& $PY/scripts/pip install --force-reinstall ../target/wheels/pybex-0.2.0-cp311-cp311-win_amd64.whl \
&& $PY/python.exe test.py
