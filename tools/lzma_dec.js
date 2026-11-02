function lzmaDecode(src, onProgress) {
  let d = src[0];
  const lc = d % 9; d = (d / 9) | 0;
  const lp = d % 5; const pb = (d / 5) | 0;
  let size = 0;
  for (let i = 0; i < 8; i++) size += src[5 + i] * 2 ** (8 * i);
  const out = new Uint8Array(size);
  let ip = 14, op = 0, range = 0xffffffff, code = 0;
  for (let i = 0; i < 4; i++) code = code * 256 + src[ip++];

  const IS_MATCH = 0, IS_REP = 192, IS_REP_G0 = 204, IS_REP_G1 = 216, IS_REP_G2 = 228,
    IS_REP0_LONG = 240, POS_SLOT = 432, POS_DEC = 688, ALIGN = 803, LEN = 819,
    REP_LEN = 1333, LIT = 1847;
  const p = new Uint16Array(LIT + (0x300 << (lc + lp))).fill(1024);

  const bit = (i) => {
    const v = p[i];
    const bound = (range >>> 11) * v;
    let r;
    if (code < bound) { range = bound; p[i] = v + ((2048 - v) >> 5); r = 0; }
    else { range -= bound; code -= bound; p[i] = v - (v >> 5); r = 1; }
    if (range < 16777216) { range *= 256; code = code * 256 + src[ip++]; }
    return r;
  };
  const tree = (base, n) => {
    let m = 1;
    for (let i = 0; i < n; i++) m = (m << 1) | bit(base + m);
    return m - (1 << n);
  };
  const rtree = (base, n) => {
    let m = 1, s = 0;
    for (let i = 0; i < n; i++) { const b = bit(base + m); m = (m << 1) | b; s |= b << i; }
    return s;
  };
  const len = (base, ps) => {
    if (!bit(base)) return tree(base + 2 + ps * 8, 3);
    if (!bit(base + 1)) return 8 + tree(base + 130 + ps * 8, 3);
    return 16 + tree(base + 258, 8);
  };

  const pbMask = (1 << pb) - 1, lpMask = (1 << lp) - 1;
  let state = 0, rep0 = 0, rep1 = 0, rep2 = 0, rep3 = 0;
  let nextReport = 1 << 20;
  while (op < size) {
    if (op >= nextReport) { nextReport += 1 << 20; if (onProgress) onProgress(op, size); }
    const ps = op & pbMask;
    if (!bit(IS_MATCH + (state << 4) + ps)) {
      const prev = op ? out[op - 1] : 0;
      const base = LIT + 0x300 * (((op & lpMask) << lc) + (prev >> (8 - lc)));
      let sym = 1;
      if (state >= 7) {
        let mb = out[op - rep0 - 1];
        do {
          const mbit = (mb >> 7) & 1; mb <<= 1;
          const b = bit(base + ((1 + mbit) << 8) + sym);
          sym = (sym << 1) | b;
          if (mbit !== b) break;
        } while (sym < 0x100);
      }
      while (sym < 0x100) sym = (sym << 1) | bit(base + sym);
      out[op++] = sym & 255;
      state = state < 4 ? 0 : state < 10 ? state - 3 : state - 6;
      continue;
    }
    let n;
    if (bit(IS_REP + state)) {
      if (!bit(IS_REP_G0 + state)) {
        if (!bit(IS_REP0_LONG + (state << 4) + ps)) {
          state = state < 7 ? 9 : 11;
          out[op] = out[op - rep0 - 1]; op++;
          continue;
        }
      } else {
        let dist;
        if (!bit(IS_REP_G1 + state)) dist = rep1;
        else {
          if (!bit(IS_REP_G2 + state)) dist = rep2; else { dist = rep3; rep3 = rep2; }
          rep2 = rep1;
        }
        rep1 = rep0; rep0 = dist;
      }
      n = len(REP_LEN, ps);
      state = state < 7 ? 8 : 11;
    } else {
      rep3 = rep2; rep2 = rep1; rep1 = rep0;
      n = len(LEN, ps);
      state = state < 7 ? 7 : 10;
      const slot = tree(POS_SLOT + (n < 3 ? n : 3) * 64, 6);
      if (slot < 4) rep0 = slot;
      else {
        const nd = (slot >> 1) - 1;
        let dist = (2 | (slot & 1)) * 2 ** nd;
        if (slot < 14) dist += rtree(POS_DEC + dist - slot, nd);
        else {
          let direct = 0;
          for (let i = nd - 4; i > 0; i--) {
            range = range >>> 1;
            let b = 0;
            if (code >= range) { code -= range; b = 1; }
            direct = direct * 2 + b;
            if (range < 16777216) { range *= 256; code = code * 256 + src[ip++]; }
          }
          dist += direct * 16 + rtree(ALIGN, 4);
        }
        if (dist === 0xffffffff) break;
        rep0 = dist;
      }
    }
    n += 2;
    if (rep0 >= op) throw new Error("lzma: bad distance");
    const end = Math.min(op + n, size);
    while (op < end) { out[op] = out[op - rep0 - 1]; op++; }
  }
  if (onProgress) onProgress(size, size);
  return out;
}
