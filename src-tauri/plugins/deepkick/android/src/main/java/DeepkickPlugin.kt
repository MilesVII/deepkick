package com.milesseventh.deepkick.bridge

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.view.WindowManager
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.ActivityResult
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import app.tauri.Logger
import app.tauri.PermissionState
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class OpenFdArgs {
    lateinit var uri: String
    var mode: String = "r"
}

@InvokeArg
class ProgressArgs {
    var title: String = "DeepKick Lyrics"
    var text: String = ""
    var max: Int = 0
    var progress: Int = 0
}

@InvokeArg
class DoneArgs {
    var title: String = "DeepKick Lyrics"
    var text: String = ""
}

@InvokeArg
class StopArgs {
    var done: DoneArgs? = null
}

private const val PERMISSION_NOTIFICATIONS = "notifications"

@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.POST_NOTIFICATIONS], alias = PERMISSION_NOTIFICATIONS)
    ]
)
class DeepkickPlugin(private val activity: Activity) : Plugin(activity) {
    companion object {
        const val EXTRA_ACTION = "com.milesseventh.deepkick.ACTION"
        private val AUDIO_EXTENSIONS = setOf("mp3", "flac")
        private val AUDIO_MIMES = setOf("audio/mpeg", "audio/mp3", "audio/flac", "audio/x-flac", "application/x-flac")
    }

    private var webView: WebView? = null
    @Volatile private var pendingAction: String? = null
    @Volatile private var insets = doubleArrayOf(0.0, 0.0, 0.0, 0.0)

    override fun load(webView: WebView) {
        super.load(webView)
        this.webView = webView
        Notifications.activityClass = activity.javaClass
        Notifications.ensureChannels(activity)

        activity.intent?.let { consumeIntent(it) }

        // Edge-to-edge: report system bar + keyboard insets to the page instead of letting
        // the WebView be padded (WebView ignores padding anyway). The page draws its own
        // dark background under the bars, so make them transparent with light icons.
        activity.window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
        WindowCompat.setDecorFitsSystemWindows(activity.window, false)
        @Suppress("DEPRECATION")
        run {
            activity.window.statusBarColor = Color.TRANSPARENT
            activity.window.navigationBarColor = Color.TRANSPARENT
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            activity.window.isNavigationBarContrastEnforced = false
            activity.window.isStatusBarContrastEnforced = false
        }
        WindowCompat.getInsetsController(activity.window, webView).apply {
            isAppearanceLightStatusBars = false
            isAppearanceLightNavigationBars = false
        }
        ViewCompat.setOnApplyWindowInsetsListener(webView) { view, windowInsets ->
            val bars = windowInsets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
            )
            val ime = windowInsets.getInsets(WindowInsetsCompat.Type.ime())
            val density = view.resources.displayMetrics.density.toDouble()
            insets = doubleArrayOf(
                bars.top / density,
                maxOf(bars.bottom, ime.bottom) / density,
                bars.left / density,
                bars.right / density
            )
            pushInsets()
            windowInsets
        }

        // Back gesture: let the page unwind its own navigation first; when it reports
        // nothing left, send the task to the background instead of destroying the
        // activity (the Rust runtime lives on).
        if (activity is AppCompatActivity) {
            activity.onBackPressedDispatcher.addCallback(activity, object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    val wv = this@DeepkickPlugin.webView
                    if (wv == null) {
                        activity.moveTaskToBack(true)
                        return
                    }
                    wv.evaluateJavascript("window.__deepkickBack ? window.__deepkickBack() : false") { result ->
                        if (result != "true") activity.moveTaskToBack(true)
                    }
                }
            })
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        consumeIntent(intent)
    }

    private fun consumeIntent(intent: Intent) {
        val action = intent.getStringExtra(EXTRA_ACTION) ?: return
        intent.removeExtra(EXTRA_ACTION)
        pendingAction = action
        runJs("window.__deepkickLaunch && window.__deepkickLaunch()")
    }

    private fun pushInsets() {
        val i = insets
        runJs("window.__deepkickInsets && window.__deepkickInsets(${i[0]}, ${i[1]}, ${i[2]}, ${i[3]})")
    }

    private fun runJs(script: String) {
        val wv = webView ?: return
        wv.post { wv.evaluateJavascript(script, null) }
    }

    // ---- file picking -------------------------------------------------------------------

    @Command
    fun pickFiles(invoke: Invoke) {
        try {
            val intent = Intent(Intent.ACTION_OPEN_DOCUMENT)
            intent.addCategory(Intent.CATEGORY_OPENABLE)
            intent.type = "audio/*"
            intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
            intent.addFlags(
                Intent.FLAG_GRANT_READ_URI_PERMISSION
                    or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                    or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
            )
            startActivityForResult(invoke, intent, "onFilesPicked")
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Failed to open the file picker")
        }
    }

    @ActivityCallback
    fun onFilesPicked(invoke: Invoke, result: ActivityResult) {
        val data = result.data
        if (result.resultCode != Activity.RESULT_OK || data == null) {
            resolveFiles(invoke, emptyList())
            return
        }
        val uris = mutableListOf<Uri>()
        val clip = data.clipData
        if (clip != null) {
            for (i in 0 until clip.itemCount) uris.add(clip.getItemAt(i).uri)
        } else {
            data.data?.let { uris.add(it) }
        }
        Thread {
            val files = uris.mapNotNull { uri ->
                persist(uri)
                describe(uri)
            }
            resolveFiles(invoke, files)
        }.start()
    }

    @Command
    fun pickDirectory(invoke: Invoke) {
        try {
            val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
            intent.addFlags(
                Intent.FLAG_GRANT_READ_URI_PERMISSION
                    or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                    or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION
            )
            startActivityForResult(invoke, intent, "onDirectoryPicked")
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "Failed to open the folder picker")
        }
    }

    @ActivityCallback
    fun onDirectoryPicked(invoke: Invoke, result: ActivityResult) {
        val tree = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || tree == null) {
            resolveFiles(invoke, emptyList())
            return
        }
        Thread {
            try {
                persist(tree)
                val rootId = DocumentsContract.getTreeDocumentId(tree)
                val rootName = rootId.substringAfterLast(':').substringAfterLast('/').ifEmpty { rootId }
                val out = mutableListOf<JSObject>()
                walk(tree, rootId, rootName, out, 0)
                out.sortWith(compareBy({ it.optString("folder") }, { it.optString("name") }))
                resolveFiles(invoke, out)
            } catch (ex: Exception) {
                Logger.error("DeepKick: failed to enumerate tree: ${ex.message}")
                invoke.reject(ex.message ?: "Failed to read the folder")
            }
        }.start()
    }

    private fun walk(tree: Uri, docId: String, path: String, out: MutableList<JSObject>, depth: Int) {
        if (depth > 48) return
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, docId)
        val projection = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_SIZE
        )
        val cursor = activity.contentResolver.query(children, projection, null, null, null) ?: return
        cursor.use {
            while (it.moveToNext()) {
                val id = it.getString(0) ?: continue
                val name = it.getString(1) ?: ""
                val mime = it.getString(2) ?: ""
                val size = if (it.isNull(3)) 0L else it.getLong(3)
                if (mime == DocumentsContract.Document.MIME_TYPE_DIR) {
                    walk(tree, id, "$path/$name", out, depth + 1)
                } else if (isAudio(name, mime)) {
                    val uri = DocumentsContract.buildDocumentUriUsingTree(tree, id)
                    out.add(entry(uri.toString(), name, path, size, mime))
                }
            }
        }
    }

    private fun isAudio(name: String, mime: String): Boolean {
        val ext = name.substringAfterLast('.', "").lowercase()
        return ext in AUDIO_EXTENSIONS || mime.lowercase() in AUDIO_MIMES
    }

    private fun persist(uri: Uri) {
        try {
            activity.contentResolver.takePersistableUriPermission(
                uri,
                Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            )
        } catch (_: Exception) {
            // not every provider offers persistable grants; the temporary grant still works
        }
    }

    private fun describe(uri: Uri): JSObject? {
        var name = uri.lastPathSegment ?: ""
        var size = 0L
        try {
            activity.contentResolver.query(
                uri,
                arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE),
                null, null, null
            )?.use {
                if (it.moveToFirst()) {
                    if (!it.isNull(0)) name = it.getString(0)
                    if (!it.isNull(1)) size = it.getLong(1)
                }
            }
        } catch (ex: Exception) {
            Logger.warn("DeepKick: cannot describe $uri: ${ex.message}")
        }
        val folder = try {
            folderLabel(DocumentsContract.getDocumentId(uri))
        } catch (_: Exception) {
            ""
        }
        val mime = try { activity.contentResolver.getType(uri) } catch (_: Exception) { null }
        return entry(uri.toString(), name, folder, size, mime ?: "")
    }

    /**
     * Turns a document id such as `primary:Music/a.mp3`, `raw:/storage/emulated/0/Download/a.mp3`
     * or `1234-ABCD:Music/a.mp3` into a short location hint.
     */
    private fun folderLabel(docId: String): String {
        if (!docId.contains(':')) return ""
        val volume = docId.substringBefore(':')
        var path = docId.substringAfter(':')
        if (volume == "raw") {
            path = path.removePrefix("/storage/emulated/0/").removePrefix("/sdcard/").removePrefix("/")
        }
        val parent = path.substringBeforeLast('/', "")
        return when {
            volume == "primary" || volume == "raw" || volume == "home" -> parent
            // media-store backed ids from the Downloads provider carry no path at all
            volume == "msf" || volume == "downloads" -> "Download"
            parent.isEmpty() -> volume
            else -> "$volume/$parent"
        }
    }

    private fun entry(uri: String, name: String, folder: String, size: Long, mime: String): JSObject {
        val o = JSObject()
        o.put("uri", uri)
        o.put("name", name)
        o.put("folder", folder)
        o.put("size", size)
        o.put("mime", mime)
        return o
    }

    private fun resolveFiles(invoke: Invoke, files: List<JSObject>) {
        val arr = JSArray()
        for (f in files) arr.put(f)
        val ret = JSObject()
        ret.put("files", arr)
        invoke.resolve(ret)
    }

    // ---- file descriptors ---------------------------------------------------------------

    @Command
    fun openFd(invoke: Invoke) {
        val args = invoke.parseArgs(OpenFdArgs::class.java)
        try {
            val pfd = activity.contentResolver.openFileDescriptor(Uri.parse(args.uri), args.mode)
                ?: throw IllegalStateException("provider returned no descriptor")
            val fd = pfd.detachFd()
            val ret = JSObject()
            ret.put("fd", fd)
            invoke.resolve(ret)
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "cannot open document")
        }
    }

    // ---- foreground service + notifications ---------------------------------------------

    @Command
    fun serviceStart(invoke: Invoke) {
        val args = invoke.parseArgs(ProgressArgs::class.java)
        try {
            val intent = Intent(activity, DeepkickService::class.java)
            intent.putExtra("title", args.title)
            intent.putExtra("text", args.text)
            intent.putExtra("max", args.max)
            intent.putExtra("progress", args.progress)
            ContextCompat.startForegroundService(activity, intent)
            invoke.resolve()
        } catch (ex: Exception) {
            invoke.reject(ex.message ?: "cannot start the foreground service")
        }
    }

    @Command
    fun serviceUpdate(invoke: Invoke) {
        val args = invoke.parseArgs(ProgressArgs::class.java)
        Notifications.notifyProgress(activity, args.title, args.text, args.max, args.progress)
        invoke.resolve()
    }

    @Command
    fun serviceStop(invoke: Invoke) {
        val args = invoke.parseArgs(StopArgs::class.java)
        try {
            activity.stopService(Intent(activity, DeepkickService::class.java))
        } catch (ex: Exception) {
            Logger.warn("DeepKick: stopService failed: ${ex.message}")
        }
        args.done?.let { Notifications.notifyDone(activity, it.title, it.text) }
        invoke.resolve()
    }

    @Command
    fun requestNotifications(invoke: Invoke) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU
            || getPermissionState(PERMISSION_NOTIFICATIONS) == PermissionState.GRANTED
        ) {
            resolveGranted(invoke, true)
            return
        }
        requestPermissionForAlias(PERMISSION_NOTIFICATIONS, invoke, "onNotificationsPermission")
    }

    @PermissionCallback
    fun onNotificationsPermission(invoke: Invoke) {
        resolveGranted(invoke, getPermissionState(PERMISSION_NOTIFICATIONS) == PermissionState.GRANTED)
    }

    private fun resolveGranted(invoke: Invoke, granted: Boolean) {
        val ret = JSObject()
        ret.put("granted", granted)
        invoke.resolve(ret)
    }

    // ---- misc ---------------------------------------------------------------------------

    @Command
    fun takeLaunchAction(invoke: Invoke) {
        val action = pendingAction
        pendingAction = null
        val ret = JSObject()
        ret.put("action", action)
        invoke.resolve(ret)
    }

    @Command
    fun getInsets(invoke: Invoke) {
        val i = insets
        val ret = JSObject()
        ret.put("top", i[0])
        ret.put("bottom", i[1])
        ret.put("left", i[2])
        ret.put("right", i[3])
        invoke.resolve(ret)
    }
}
