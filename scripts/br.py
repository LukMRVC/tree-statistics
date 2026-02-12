def initialize_fkp(zero_k: int, max_k: int, max_p: int):
  fkp = []
  # Maps logical p=-1 to index 0, p=0 to index 1, etc.
  for p in range(-1, max_p + 1): 
      fkp.append([-1] * (max_k + 1))

  for k in range(-zero_k, max_k - zero_k):
      for p in range(-1, max_p + 1): # CHANGE 1: Match range above
          if p == abs(k) - 1:
              if k < 0:
                  fkp[p + 1][k + zero_k] = abs(k) - 1
              else:
                  fkp[p + 1][k + zero_k] = -1
          else:
              fkp[p + 1][k + zero_k] = -999 # Represent -infinity
  return fkp


def print_fkp(fkp, zero_k: int):
  print(f"diag:  ", end="")
  max_k = len(fkp[0])
  for k in range(-zero_k, max_k - zero_k):
    print(f"{k:>5}", end="")
  print()
  
  for p, p_list in enumerate(fkp):
    print(f"p={p - 1:<3}: ", end="")
    for k, k_list in enumerate(p_list):
      print(f"{k_list:>5}", end="")
    print()


def br(a: str, b: str, max_cost: int) -> int:
  global fkp
  global ZERO_K_OFFSET
  
  assert len(b) >= len(a)
  
  fkp_called = 0
  
  n = len(b)
  m = len(a)
  
  size_diff = len(b) - len(a)
  target_diag = size_diff
  
  if size_diff > max_cost:
    return -1
  
  
  def calc_f(diag: int, cost: int):
    nonlocal fkp_called
    fkp_called += 1
    global fkp
    prev_row = fkp[cost]
    
    # Standard Levenshtein recurrence (Insertion, Deletion, Substitution)
    t = max(
        prev_row[diag + ZERO_K_OFFSET] + 1,     # Substitution
        prev_row[diag - 1 + ZERO_K_OFFSET],     # Deletion (from k-1)
        prev_row[diag + 1 + ZERO_K_OFFSET] + 1  # Insertion (from k+1)
    )
    
    # While loop to extend the match (Ukkonen's optimization)
    # Added safe bounds check (t >= 0) just in case initialization used -999
    while (t < m and 
               (t + diag) < n and 
               t >= 0 and 
               (t + diag) >= 0 and 
               a[t] == b[t + diag]):
            t += 1
    fkp[cost + 1][diag + ZERO_K_OFFSET] = t
  
  p = target_diag
  
  while True:
    inc = p
    
    for temp_p in range(0, p):
      if abs((n - m) - inc) <= temp_p:
        calc_f((n - m) - inc, temp_p)
      if abs((n - m) + inc) <= temp_p:
        calc_f((n - m) + inc, temp_p)
        
      inc -= 1
    
    calc_f((n - m), p)
    p += 1
    
    if fkp[p][target_diag + ZERO_K_OFFSET] == m:
      print(f"Total fkp calls: {fkp_called}")
      return p - 1
    elif p > max_cost:
      print(f"Total fkp calls: {fkp_called}")
      return -1
      
  
    
if __name__ == "__main__":
  MAX_K = 20
  MAX_P = 10
  ZERO_K_OFFSET = MAX_K // 2
  fkp = initialize_fkp(ZERO_K_OFFSET, MAX_K, MAX_P + 2)
  print_fkp(fkp, ZERO_K_OFFSET)
  print('Resulting distance is:', br("avery", "garvey", 3))
  print('Resulting distance is:', br("avery", "garvey", 2))
  print('Resulting distance is:', br("abcde", "fghij", 5))
  
  print('Resulting distance is:', br("kitten", "sitting", 3))
  