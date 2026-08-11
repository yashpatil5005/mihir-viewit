package ai.viewit.plugins.iworkuniversal;
import android.content.Context; import android.net.Uri;
import ai.viewit.app.ViewItDocumentPlugin;
import java.io.*; import java.util.*;
public class IworkUniversalPlugin implements ViewItDocumentPlugin {
  private Context context; private static boolean loaded=false;
  public String getId(){return "iwork-universal";} public String getVersion(){return "0.1.0";}
  public List<String> getSupportedFormats(){return Arrays.asList("pages","numbers","key");}
  public void initialize(Context c){context=c.getApplicationContext();if(loaded)return;for(File r:Arrays.asList(new File(context.getFilesDir(),"plugins/iwork-universal.staging/lib"),new File(context.getFilesDir(),"plugins/iwork-universal/lib"))){for(String a:android.os.Build.SUPPORTED_ABIS){File f=new File(r,a+"/libviewit_plugin_iwork_universal.so");if(f.exists())try{System.load(f.getAbsolutePath());loaded=true;return;}catch(UnsatisfiedLinkError ignored){}}}throw new UnsatisfiedLinkError("No iwork-universal native library");}
  public boolean canHandle(String m){return m!=null&&(m.contains("iwork")||m.contains("pages")||m.contains("numbers")||m.contains("keynote"));}
  public boolean canHandleExt(String e){return e!=null&&getSupportedFormats().contains(e.toLowerCase());}
  public String render(Uri uri,String ext){try{InputStream in=context.getContentResolver().openInputStream(uri);if(in==null&&"file".equals(uri.getScheme()))in=new FileInputStream(new File(uri.getPath()));ByteArrayOutputStream out=new ByteArrayOutputStream();byte[] b=new byte[8192];for(int n;(n=in.read(b))>=0;)out.write(b,0,n);in.close();return nativeRenderBytes(out.toByteArray(),ext.toLowerCase());}catch(Exception e){return "{\"kind\":\"unsupported\",\"reason\":\""+e.getMessage()+"\",\"suggestion\":\"none\"}";}}
  public void cleanup(){nativeCleanup();}
  private static native boolean nativeCanHandleMimeType(String m); private static native boolean nativeCanHandleExt(String e); private static native String nativeRenderBytes(byte[] b,String e); private static native void nativeCleanup();
}
