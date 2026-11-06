# RAM-only application for unmodified official CoreMatrix UIFlow.
# Hash vectors are independently generated; HMAC RFC 4231 and AES FIPS 197 / SP800-38A.
import hashlib
import binascii
import cryptolib
import uctypes

failures = []


def verify(name, actual, expected):
    result = binascii.hexlify(actual).decode()
    print('CM_CRYPTO_RESULT', name, result)
    if result != expected:
        failures.append(name)
        print('CM_CRYPTO_MISMATCH', name, expected)


def hmac256(key, message):
    if len(key) > 64:
        key = hashlib.sha256(key).digest()
    key += b'\x00' * (64 - len(key))
    inner = bytes(x ^ 0x36 for x in key)
    outer = bytes(x ^ 0x5c for x in key)
    return hashlib.sha256(outer + hashlib.sha256(inner + message).digest()).digest()


vectors = {'empty': {'sha256': 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855', 'sha1': 'da39a3ee5e6b4b0d3255bfef95601890afd80709', 'md5': 'd41d8cd98f00b204e9800998ecf8427e'}, 'abc': {'sha256': 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad', 'sha1': 'a9993e364706816aba3e25717850c26c9cd0d89d', 'md5': '900150983cd24fb0d6963f7d28e17f72'}, 'nist-multiblock': {'sha256': '248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1', 'sha1': '84983e441c3bd26ebaae4aa1f95129e5e54670f1', 'md5': '8215ef0796a20bcaaae116d3876c664a'}, '55': {'sha256': '463eb28e72f82e0a96c0a4cc53690c571281131f672aa229e0d45ae59b598b59', 'sha1': '8ae2d46729cfe68ff927af5eec9c7d1b66d65ac2', 'md5': '6912ee65fff2d9f9ce2508cddf8bcda0'}, '56': {'sha256': 'da2ae4d6b36748f2a318f23e7ab1dfdf45acdc9d049bd80e59de82a60895f562', 'sha1': '636e2ec698dac903498e648bd2f3af641d3c88cb', 'md5': '51fdd1acda72405dfdfa03fcb85896d7'}, '63': {'sha256': '29af2686fd53374a36b0846694cc342177e428d1647515f078784d69cdb9e488', 'sha1': '6d942da0c4392b123528f2905c713a3ce28364bd', 'md5': '48a6295221902e8e0938f773a7185e72'}, '64': {'sha256': 'fdeab9acf3710362bd2658cdc9a29e8f9c757fcf9811603a8c447cd1d9151108', 'sha1': 'c6138d514ffa2135bfce0ed0b8fac65669917ec7', 'md5': 'b2d3f56bc197fd985d5965079b5e7148'}, '65': {'sha256': '4bfd2c8b6f1eec7a2afeb48b934ee4b2694182027e6d0fc075074f2fabb31781', 'sha1': '69bd728ad6e13cd76ff19751fde427b00e395746', 'md5': '8bd7053801c768420faf816fadba971c'}, '119': {'sha256': 'da18797ed7c3a777f0847f429724a2d8cd5138e6ed2895c3fa1a6d39d18f7ec6', 'sha1': '41c89d06001bab4ab78736b44efe7ce18ce6ae08', 'md5': '1c772251899a7ff007400b888d6b2042'}, '120': {'sha256': 'f52b23db1fbb6ded89ef42a23ce0c8922c45f25c50b568a93bf1c075420bbb7c', 'sha1': 'd3dbd653bd8597b7475321b60a36891278e6a04a', 'md5': 'b7ba1efc6022e9ed272f00b8831e26e6'}, '127': {'sha256': '92ca0fa6651ee2f97b884b7246a562fa71250fedefe5ebf270d31c546bfea976', 'sha1': '89d7312a903f65cd2b3e34a975e55dbea9033353', 'md5': '8402b21e7bc7906493bae0dac017f1f9'}, '128': {'sha256': '471fb943aa23c511f6f72f8d1652d9c880cfa392ad80503120547703e56a2be5', 'sha1': 'e6434bc401f98603d7eda504790c98c67385d535', 'md5': '37eff01866ba3f538421b30b7cbefcac'}, '129': {'sha256': '5099c6a56203f9687f7d33f4bfdf576d31dc91f6b695ecea38b2770c87631135', 'sha1': '3352e41cc30b40ae80108970492b21014049e625', 'md5': '46f986692847558fc38b0cece591c20f'}, '3968': {'sha256': 'cf8b540956989763fa73148cbb6dfd9ac3a439a31d15cab714e9e1f7466553ce', 'sha1': '55c8b6f9e401cac450113d776c3911e52fb23768', 'md5': '9a4278498f0bb58aa713233800864cc7'}, '4095': {'sha256': '02eb925ab7d50a3333adc330ce64d365f863e21a4c9605b5455211c65f600cb1', 'sha1': '4b892ef6a50b100eb11f74e3f2daaefd22417fca', 'md5': '4f059904fb7cb387d3903dcfd9dd2a7c'}, '4096': {'sha256': 'c8f5d0341d54d951a71b136e6e2afcb14d11ed8489a7ae126a8fee0df6ecf193', 'sha1': 'e9dded8c84614e894501965af60c2525794a8c7d', 'md5': '2bcd3c4de20c918e19fab5c36249c70d'}, '4097': {'sha256': '1e973d029df2b2c66cb42a942c5edb45966f02abaff29fe99410e44d271d0efc', 'sha1': 'c627e736efd8bb0dff1778335c9c79cb1f27e396', 'md5': '70410aad262cd11e63ae854804c8024b'}, '8192': {'sha256': 'dc404a613fedaeb54034514bc6505f56b933caa5250299ba7d094377a51caa46', 'sha1': 'ecca46e1a1d0a6012713b09a870d84f695b6d9b0', 'md5': '6556112372898c69e1de0bf689d8db26'}, '16384': {'sha256': 'a1f259d4365ed4320c377ce26f5c8c56dcdc9a89e7b641bfd8eabfbbeac86654', 'sha1': '80cb9c430d80c3084649f65e0ca25dabbffb1b62', 'md5': '3df67097cee5e4cea36e0f941c134ffc'}}
for name in ('empty', 'abc', 'nist-multiblock', '55', '56', '63', '64', '65', '119', '120', '127', '128', '129', '3968', '4095', '4096', '4097', '8192', '16384'):
    if name == 'empty':
        data = b''
    elif name == 'abc':
        data = b'abc'
    elif name == 'nist-multiblock':
        data = b'abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq'
    else:
        count = int(name)
        data = bytes(range(256)) * (count // 256) + bytes(range(count % 256))
    print('CM_CRYPTO_INPUT', name, len(data), hex(uctypes.addressof(data)))
    for algorithm in ('sha256', 'sha1', 'md5'):
        factory = getattr(hashlib, algorithm)
        verify(algorithm + '/' + name + '/oneshot', factory(data).digest(), vectors[name][algorithm])
        if name in ('65', '4097', '16384'):
            for chunk in ((1, 63, 64, 65) if len(data) <= 65 else (63, 64, 65, 1024, 4096)):
                digest = factory()
                for offset in range(0, len(data), chunk):
                    digest.update(data[offset:offset + chunk])
                verify(algorithm + '/' + name + '/chunk' + str(chunk), digest.digest(), vectors[name][algorithm])
print('CM_CRYPTO_HASH_DONE', len(failures))

for name, key, data, expected in (
    ('rfc4231-1', b'\x0b' * 20, b'Hi There', 'b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7'),
    ('rfc4231-2', b'Jefe', b'what do ya want for nothing?', '5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843'),
    ('rfc4231-6', b'\xaa' * 131, b'Test Using Larger Than Block-Size Key - Hash Key First', '60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54')
):
    verify('hmac/' + name, hmac256(key, data), expected)
print('CM_CRYPTO_HMAC_DONE', len(failures))

# RFC 5246 section 5 P_SHA256 composition; public fixed input, host hmac oracle.
secret = b'CoreMatrix public TLS12 PRF vector'
seed = b'key expansion' + bytes(range(64))
a = seed
output = b''
while len(output) < 128:
    a = hmac256(secret, a)
    output += hmac256(secret, a + seed)
verify('tls12-p-sha256/128', output[:128], 'ca842c38215ad8bc0ea226ff51117c4b0fab376b9887613d6dbca60468f7aefad3ba7ade0a2bd67aba173d92febcc171235cce880f616715183b39bc0853de5fa783b8d8a703bb3ea3c419614c1363b78c756b57c0d21e97f8808672bc14ba5e37a6915134e0e2cd0cdb5935dbae87fcee96dd8efe905ba20ac7abb53a994538')
print('CM_CRYPTO_PRF_DONE', len(failures))

unhex = binascii.unhexlify
plaintext = unhex('00112233445566778899aabbccddeeff')
for key, ciphertext in (
    ('000102030405060708090a0b0c0d0e0f', '69c4e0d86a7b0430d8cdb78070b4c55a'),
    ('000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f', '8ea2b7ca516745bfeafc49904b496089')
):
    key = unhex(key)
    verify('aes' + str(len(key) * 8) + '/ecb/encrypt', cryptolib.aes(key, 1).encrypt(plaintext), ciphertext)
    verify('aes' + str(len(key) * 8) + '/ecb/decrypt', cryptolib.aes(key, 1).decrypt(unhex(ciphertext)), binascii.hexlify(plaintext).decode())

key = unhex('2b7e151628aed2a6abf7158809cf4f3c')
plain_hex = '6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e5130c81c46a35ce411e5fbc1191a0a52eff69f2445df4f9b17ad2b417be66c3710'
plaintext = unhex(plain_hex)
for name, mode, iv, ciphertext in (
    ('cbc', 2, '000102030405060708090a0b0c0d0e0f', '7649abac8119b246cee98e9b12e9197d5086cb9b507219ee95db113a917678b273bed6b8e3c1743b7116e69e222295163ff1caa1681fac09120eca307586e1a7'),
    ('ctr', 6, 'f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff', '874d6191b620e3261bef6864990db6ce9806f66b7970fdff8617187bb9fffdff5ae4df3edbd5d35e5b4f09020db03eab1e031dda2fbe03d1792170a0f3009cee')
):
    try:
        cipher = cryptolib.aes(key, mode, unhex(iv))
    except ValueError as error:
        if mode == 6 and str(error) == 'mode':
            print('CM_CRYPTO_API_UNAVAILABLE', 'cryptolib CTR mode6')
            continue
        raise
    verify('aes128/' + name + '/encrypt', cipher.encrypt(plaintext), ciphertext)
    verify('aes128/' + name + '/decrypt', cryptolib.aes(key, mode, unhex(iv)).decrypt(unhex(ciphertext)), plain_hex)
print('CM_CRYPTO_AES_DONE', len(failures))
assert not failures, failures
print('CM_CRYPTO_PASS')
