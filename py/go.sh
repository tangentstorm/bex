if [ "${PWD##*/}" != "py" ]; then
  cd py
fi
export PY=c:/python311
maturin build -i $PY/python.exe \
&& $PY/scripts/pip install --force-reinstall ../target/wheels/bex-*-cp311-cp311-win_amd64.whl \
&& $PY/python.exe test.py
