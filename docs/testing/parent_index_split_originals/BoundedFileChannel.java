import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.MappedByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.channels.ReadableByteChannel;
import java.nio.channels.WritableByteChannel;

/** Own delegating file bound; no database format interpretation or patching. */
final class BoundedFileChannel extends FileChannel {
    private final FileChannel inner;
    private final long maximum;

    BoundedFileChannel(FileChannel inner, long maximum) throws IOException {
        this.inner = inner;
        this.maximum = maximum;
        if (inner.size() > maximum) {
            throw new IOException("Initial file exceeds the producer envelope");
        }
    }

    private void requireWrite(long position, long count) throws IOException {
        if (position < 0 || count < 0 || position > maximum || count > maximum - position) {
            throw new IOException("Producer file ceiling reached");
        }
    }

    @Override
    public int read(ByteBuffer target) throws IOException {
        return inner.read(target);
    }

    @Override
    public int read(ByteBuffer target, long position) throws IOException {
        return inner.read(target, position);
    }

    @Override
    public long read(ByteBuffer[] targets, int offset, int length) throws IOException {
        return inner.read(targets, offset, length);
    }

    @Override
    public int write(ByteBuffer source) throws IOException {
        requireWrite(inner.position(), source.remaining());
        return inner.write(source);
    }

    @Override
    public int write(ByteBuffer source, long position) throws IOException {
        requireWrite(position, source.remaining());
        return inner.write(source, position);
    }

    @Override
    public long write(ByteBuffer[] sources, int offset, int length) throws IOException {
        long total = 0;
        for (int index = offset; index < offset + length; index++) {
            total = Math.addExact(total, sources[index].remaining());
        }
        requireWrite(inner.position(), total);
        return inner.write(sources, offset, length);
    }

    @Override
    public long position() throws IOException {
        return inner.position();
    }

    @Override
    public FileChannel position(long position) throws IOException {
        requireWrite(position, 0);
        inner.position(position);
        return this;
    }

    @Override
    public long size() throws IOException {
        return inner.size();
    }

    @Override
    public FileChannel truncate(long size) throws IOException {
        requireWrite(0, size);
        inner.truncate(size);
        return this;
    }

    @Override
    public void force(boolean metadata) throws IOException {
        inner.force(metadata);
    }

    @Override
    public long transferTo(long position, long count, WritableByteChannel target) throws IOException {
        return inner.transferTo(position, count, target);
    }

    @Override
    public long transferFrom(ReadableByteChannel source, long position, long count) throws IOException {
        requireWrite(position, count);
        return inner.transferFrom(source, position, count);
    }

    @Override
    public MappedByteBuffer map(MapMode mode, long position, long size) throws IOException {
        if (mode != MapMode.READ_ONLY) {
            throw new IOException("Writable mappings bypass the producer ceiling");
        }
        return inner.map(mode, position, size);
    }

    @Override
    public FileLock lock(long position, long size, boolean shared) throws IOException {
        return inner.lock(position, size, shared);
    }

    @Override
    public FileLock tryLock(long position, long size, boolean shared) throws IOException {
        return inner.tryLock(position, size, shared);
    }

    @Override
    protected void implCloseChannel() throws IOException {
        inner.close();
    }
}
