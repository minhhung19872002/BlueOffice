using System.Buffers;
using System.Text;

namespace BlueOffice.Api.Collab;

/// <summary>An awareness entry ([y-protocols] awareness update: clientID, clock, JSON state; "null" = removed).</summary>
public readonly record struct AwarenessEntry(ulong ClientId, ulong Clock, string State);

/// <summary>
/// Framing of y-protocols / y-websocket messages (lib0 encoding). Only the envelope is decoded:
/// Yjs updates stay opaque byte arrays.
/// <code>
/// message      = varUint(messageType) body
/// sync body    = varUint(syncType: 0 step1 | 1 step2 | 2 update) varUint8Array(payload)
/// awareness    = varUint8Array(varUint(count) { varUint(clientID) varUint(clock) varString(stateJSON) })
/// queryAwareness = (empty)
/// </code>
/// </summary>
public static class YProtocol
{
    /// <summary>messageSync.</summary>
    public const ulong MessageSync = 0;

    /// <summary>messageAwareness.</summary>
    public const ulong MessageAwareness = 1;

    /// <summary>messageAuth.</summary>
    public const ulong MessageAuth = 2;

    /// <summary>messageQueryAwareness.</summary>
    public const ulong MessageQueryAwareness = 3;

    /// <summary>Sync step 1 (state vector).</summary>
    public const ulong SyncStep1 = 0;

    /// <summary>Sync step 2 (missing updates).</summary>
    public const ulong SyncStep2 = 1;

    /// <summary>Incremental update.</summary>
    public const ulong SyncUpdate = 2;

    /// <summary>Encoded state vector of an empty document (asks the peer for its whole state).</summary>
    public static readonly byte[] EmptyStateVector = [0];

    /// <summary>Encoded (v1) update of an empty document: 0 structs, empty delete set.</summary>
    public static readonly byte[] EmptyUpdate = [0, 0];

    /// <summary>Reads a lib0 varUint (max 64 bits).</summary>
    public static ulong ReadVarUint(ReadOnlySpan<byte> data, ref int pos)
    {
        ulong value = 0;
        var shift = 0;
        while (true)
        {
            if (pos >= data.Length)
            {
                throw new FormatException("Truncated varUint.");
            }

            var b = data[pos++];
            value |= (ulong)(b & 0x7F) << shift;
            if (b < 0x80)
            {
                return value;
            }

            shift += 7;
            if (shift > 63)
            {
                throw new FormatException("varUint too long.");
            }
        }
    }

    /// <summary>Reads a lib0 varUint8Array (length-prefixed bytes).</summary>
    public static byte[] ReadVarUint8Array(ReadOnlySpan<byte> data, ref int pos)
    {
        var len = ReadVarUint(data, ref pos);
        if (len > (ulong)(data.Length - pos))
        {
            throw new FormatException("Truncated varUint8Array.");
        }

        var result = data.Slice(pos, (int)len).ToArray();
        pos += (int)len;
        return result;
    }

    /// <summary>Reads a lib0 varString (UTF-8, length-prefixed).</summary>
    public static string ReadVarString(ReadOnlySpan<byte> data, ref int pos) => Encoding.UTF8.GetString(ReadVarUint8Array(data, ref pos));

    /// <summary>Writes a varUint.</summary>
    public static void WriteVarUint(IBufferWriter<byte> w, ulong value)
    {
        while (value >= 0x80)
        {
            w.GetSpan(1)[0] = (byte)(0x80 | (value & 0x7F));
            w.Advance(1);
            value >>= 7;
        }

        w.GetSpan(1)[0] = (byte)value;
        w.Advance(1);
    }

    /// <summary>Writes a varUint8Array.</summary>
    public static void WriteVarUint8Array(IBufferWriter<byte> w, ReadOnlySpan<byte> bytes)
    {
        WriteVarUint(w, (ulong)bytes.Length);
        w.Write(bytes);
    }

    /// <summary>Encodes a sync message.</summary>
    public static byte[] EncodeSync(ulong syncType, ReadOnlySpan<byte> payload)
    {
        var w = new ArrayBufferWriter<byte>(payload.Length + 8);
        WriteVarUint(w, MessageSync);
        WriteVarUint(w, syncType);
        WriteVarUint8Array(w, payload);
        return w.WrittenSpan.ToArray();
    }

    /// <summary>Encodes an awareness message.</summary>
    public static byte[] EncodeAwareness(IReadOnlyCollection<AwarenessEntry> entries)
    {
        var inner = new ArrayBufferWriter<byte>();
        WriteVarUint(inner, (ulong)entries.Count);
        foreach (var e in entries)
        {
            WriteVarUint(inner, e.ClientId);
            WriteVarUint(inner, e.Clock);
            WriteVarUint8Array(inner, Encoding.UTF8.GetBytes(e.State));
        }

        var w = new ArrayBufferWriter<byte>(inner.WrittenCount + 8);
        WriteVarUint(w, MessageAwareness);
        WriteVarUint8Array(w, inner.WrittenSpan);
        return w.WrittenSpan.ToArray();
    }

    /// <summary>Decodes the entries of an awareness update payload.</summary>
    public static List<AwarenessEntry> DecodeAwareness(ReadOnlySpan<byte> update)
    {
        var pos = 0;
        var count = ReadVarUint(update, ref pos);
        var list = new List<AwarenessEntry>();
        for (ulong i = 0; i < count; i++)
        {
            var id = ReadVarUint(update, ref pos);
            var clock = ReadVarUint(update, ref pos);
            list.Add(new AwarenessEntry(id, clock, ReadVarString(update, ref pos)));
        }

        return list;
    }

    /// <summary>Serializes a list of updates as <c>{varUint8Array}</c>* (persistence format).</summary>
    public static byte[] EncodeFrames(IEnumerable<byte[]> frames)
    {
        var w = new ArrayBufferWriter<byte>();
        foreach (var f in frames)
        {
            WriteVarUint8Array(w, f);
        }

        return w.WrittenSpan.ToArray();
    }

    /// <summary>Inverse of <see cref="EncodeFrames"/>.</summary>
    public static List<byte[]> DecodeFrames(ReadOnlySpan<byte> data)
    {
        var list = new List<byte[]>();
        var pos = 0;
        while (pos < data.Length)
        {
            list.Add(ReadVarUint8Array(data, ref pos));
        }

        return list;
    }
}
