package rust.jniminhelper;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;

public class InvocHdl implements InvocationHandler {
    long rust_hdl_id;

    // to be registered in native code
    private native Object rustHdl(long id, Method method, Object[] args) throws Throwable;

    public InvocHdl(long id) {
        this.rust_hdl_id = id;
    }

    public long getId() {
        return this.rust_hdl_id;
    }

    @Override
    public Object invoke(Object proxy, Method method, Object[] args) throws Throwable {
        String methodName = method.getName();
        if (methodName.equals("equals")) {
            if (args == null || args.length == 0) {
                return Boolean.FALSE;
            }
            Object other = args[0];
            if (other == proxy) return Boolean.TRUE;
            if (other == null) return Boolean.FALSE;
            if (Proxy.isProxyClass(other.getClass())) {
                InvocationHandler otherH = Proxy.getInvocationHandler(other);
                if (otherH instanceof InvocHdl) {
                    return Boolean.valueOf(this.rust_hdl_id == ((InvocHdl) otherH).rust_hdl_id);
                }
            }
            return Boolean.FALSE;
        }
        if (methodName.equals("hashCode")) {
            // mix high and low 32 bits of the long
            int h = (int) (rust_hdl_id ^ (rust_hdl_id >>> 32));
            return Integer.valueOf(h);
        }
        if (methodName.equals("toString")) {
            return "rust.jniminhelper.InvocHdl[" + this.rust_hdl_id + "]";
        }
        return rustHdl(this.rust_hdl_id, method, args);
    }
}
