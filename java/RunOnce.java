// Currently this is built for usage in `post_to_main_looper`.

package rust.jniminhelper;

import java.lang.Runnable;
import java.util.concurrent.atomic.AtomicBoolean;

public class RunOnce implements Runnable {
    private long rust_hdl_id;
    private Object life_checker;

    // NOTE: the use of atomic boolean here is required for the soundness of
    // the Rust side code.
    private final AtomicBoolean is_valid = new AtomicBoolean(true);

    // to be registered in native code
    private native void rustHdl(long id, Object checker) throws Throwable;

    public RunOnce(long id, Object checker) {
        this.rust_hdl_id = id;
        this.life_checker = checker;
    }

    public void markInvalid() {
        this.is_valid.set(false);
    }

    @Override
    public void run() {
        if (! this.is_valid.getAndSet(false)) {
            return;
        }
        long id = this.rust_hdl_id;
        this.rust_hdl_id = 0;
        try {
            this.rustHdl(id, this.life_checker);
        } catch (Throwable ex) {}
    }
}
