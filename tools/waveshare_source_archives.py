"""Read retained manufacturer RAR members without extracting paths to disk.

Uses the system libarchive library. Original archive and member hashes remain
the caller's responsibility; this reader also rejects duplicate member names,
special files, missing requested members and excessive decoded member sizes.
"""
import ctypes
import ctypes.util


def rar_members(data, include=None, max_member_bytes=128 * 1024 * 1024):
    library = ctypes.util.find_library('archive')
    if not library:
        raise RuntimeError('RAR source verification requires the system libarchive library')
    lib = ctypes.CDLL(library)
    signatures = {
        'archive_read_new': (ctypes.c_void_p, []),
        'archive_read_support_filter_all': (ctypes.c_int, [ctypes.c_void_p]),
        'archive_read_support_format_all': (ctypes.c_int, [ctypes.c_void_p]),
        'archive_read_open_memory': (ctypes.c_int, [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t]),
        'archive_read_next_header': (ctypes.c_int, [ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)]),
        'archive_read_data': (ctypes.c_ssize_t, [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t]),
        'archive_read_data_skip': (ctypes.c_int, [ctypes.c_void_p]),
        'archive_read_free': (ctypes.c_int, [ctypes.c_void_p]),
        'archive_error_string': (ctypes.c_char_p, [ctypes.c_void_p]),
        'archive_format_name': (ctypes.c_char_p, [ctypes.c_void_p]),
        'archive_entry_pathname_utf8': (ctypes.c_char_p, [ctypes.c_void_p]),
        'archive_entry_filetype': (ctypes.c_uint, [ctypes.c_void_p]),
        'archive_entry_size': (ctypes.c_int64, [ctypes.c_void_p]),
    }
    for name, (result, arguments) in signatures.items():
        function = getattr(lib, name)
        function.restype, function.argtypes = result, arguments
    handle = lib.archive_read_new()
    if not handle:
        raise RuntimeError('Cannot allocate libarchive reader')
    wanted = None if include is None else set(include)
    names, result = set(), {}
    memory = ctypes.create_string_buffer(data)

    def checked(code):
        if code < 0:
            message = lib.archive_error_string(handle)
            raise ValueError('RAR reader: ' + (message.decode('utf-8', 'replace') if message else str(code)))
        return code

    try:
        checked(lib.archive_read_support_filter_all(handle))
        checked(lib.archive_read_support_format_all(handle))
        checked(lib.archive_read_open_memory(handle, memory, len(data)))
        entry = ctypes.c_void_p()
        while checked(lib.archive_read_next_header(handle, ctypes.byref(entry))) != 1:
            format_name = lib.archive_format_name(handle)
            if not format_name or b'RAR' not in format_name.upper():
                raise ValueError('Expected a RAR source archive')
            raw_name = lib.archive_entry_pathname_utf8(entry)
            if raw_name is None:
                raise ValueError('RAR member has no valid UTF-8 path')
            name = raw_name.decode('utf-8')
            if name in names:
                raise ValueError('Duplicate RAR member: ' + name)
            names.add(name)
            file_type = lib.archive_entry_filetype(entry)
            if file_type == 0o040000:
                checked(lib.archive_read_data_skip(handle))
                continue
            if file_type != 0o100000:
                raise ValueError('Unsupported RAR special file: ' + name)
            size = lib.archive_entry_size(entry)
            if not 0 <= size <= max_member_bytes:
                raise ValueError('RAR member exceeds the decoded size limit: ' + name)
            if wanted is not None and name not in wanted:
                checked(lib.archive_read_data_skip(handle))
                continue
            chunks, count = [], 0
            buffer = ctypes.create_string_buffer(1024 * 1024)
            while True:
                length = checked(lib.archive_read_data(handle, buffer, len(buffer)))
                if length == 0:
                    break
                count += length
                if count > size or count > max_member_bytes:
                    raise ValueError('RAR decoded member exceeds declared size: ' + name)
                chunks.append(buffer.raw[:length])
            if count != size:
                raise ValueError('Truncated RAR member: ' + name)
            result[name] = b''.join(chunks)
        if wanted is not None and wanted != set(result):
            raise ValueError('Missing requested RAR member: ' + ', '.join(sorted(wanted - set(result))))
        return result
    finally:
        lib.archive_read_free(handle)
