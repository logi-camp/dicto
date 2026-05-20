// 解压缩这个地方优化一下
pub fn fast_decrypt(encrypted: &[u8], key: &[u8]) -> Vec<u8> {
    let mut buf = Vec::from(encrypted);
    let mut prev = 0x36;
    for i in 0..buf.len() {
        let mut t = buf[i].rotate_left(4);
        t = t ^ prev ^ (i as u8) ^ key[i % key.len()];
        prev = buf[i];
        buf[i] = t;
    }
    buf
}
