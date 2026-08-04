@echo off
REM First Look demo launcher — seeds demo template under %USERPROFILE%\Lattice
REM without redirecting LATTICE_DEV_HOME. See docs/dev/first-look-demo.md.
set "LATTICE_SEED_FIRST_LOOK=1"
start "" "%~dp0Lattice.exe" %*
