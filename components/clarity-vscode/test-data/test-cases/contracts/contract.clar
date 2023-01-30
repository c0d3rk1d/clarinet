(define-data-var counter int 0)

(define-read-only (get-counter)
  (ok (var-get counter))
)

(define-public (add (n int))
  (begin
    ;; (asserts! (> n 0) (err u1))
    (var-set counter (+ (var-get counter) n))
    (ok (var-get counter))
  )
)
