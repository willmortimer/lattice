;; Minimal WASIp1 guest: read /input/hello.txt and write /output/out.txt.
;; Preopen fds: /input=3, /work=4, /output=5, /tmp=6 (stdin/stdout/stderr=0..2).
(module
  (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_read" (func $fd_read (param i32 i32 i32 i32) (result i32)))
  (import "wasi_snapshot_preview1" "fd_close" (func $fd_close (param i32) (result i32)))
  (import "wasi_snapshot_preview1" "path_open"
    (func $path_open (param i32 i32 i32 i32 i32 i64 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "proc_exit" (func $proc_exit (param i32)))

  (memory 1)
  (export "memory" (memory 0))

  ;; 0: read iovec; 8: write iovec; 16: nread; 20: buffer; 1044/1054: paths
  (data (i32.const 1044) "hello.txt")
  (data (i32.const 1054) "out.txt")

  (func (export "_start")
    (local $in_fd i32)
    (local $out_fd i32)
    (local $nread i32)

    (local.set $in_fd
      (call $path_open
        (i32.const 3)
        (i32.const 0)
        (i32.const 1044)
        (i32.const 9)
        (i32.const 0)
        (i64.const 1)
        (i64.const 1)
        (i32.const 0)))

    (i32.store (i32.const 0) (i32.const 20))
    (i32.store (i32.const 4) (i32.const 1024))
    (call $fd_read
      (local.get $in_fd)
      (i32.const 0)
      (i32.const 1)
      (i32.const 16))
    drop
    (local.set $nread (i32.load (i32.const 16)))
    (call $fd_close (local.get $in_fd))
    drop

    ;; O_CREAT | O_TRUNC = 9; FD_WRITE = 2
    (local.set $out_fd
      (call $path_open
        (i32.const 5)
        (i32.const 0)
        (i32.const 1054)
        (i32.const 7)
        (i32.const 9)
        (i64.const 2)
        (i64.const 2)
        (i32.const 0)))

    (i32.store (i32.const 8) (i32.const 20))
    (i32.store (i32.const 12) (local.get $nread))
    (call $fd_write
      (local.get $out_fd)
      (i32.const 8)
      (i32.const 1)
      (i32.const 16))
    drop
    (call $fd_close (local.get $out_fd))
    drop

    (call $proc_exit (i32.const 0))
  )
)
