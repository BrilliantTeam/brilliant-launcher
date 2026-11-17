package net.brilliantw.launcher

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import net.kdt.pojavlaunch.BrilliantLaunch

@InvokeArg
class LaunchArgs {
  lateinit var versionId: String
  lateinit var gameDir: String
  lateinit var username: String
  lateinit var uuid: String
  var accessToken: String = "0"
  var xuid: String = ""
  var language: String = "zh-TW"
  var jvmArgs: String = ""
}

@InvokeArg
class StageArgs {
  lateinit var uri: String
}

@InvokeArg
class OpenFileArgs {
  lateinit var path: String
}

@InvokeArg
class KeepAliveArgs {
  var on: Boolean = false
  var title: String = ""
  var channel: String = ""
}

@InvokeArg
class LoginArgs {
  lateinit var url: String
  lateinit var redirect: String
  var title: String = ""
  var close: String = ""
}

@TauriPlugin
class GamePlugin(private val activity: Activity) : Plugin(activity) {
  private var keepingAlive = false

  @Command
  fun launch(invoke: Invoke) {
    val args = invoke.parseArgs(LaunchArgs::class.java)
    BrilliantLaunch.launch(activity, args.versionId, args.gameDir, args.username, args.uuid, args.accessToken, args.xuid, 0, args.language, args.jvmArgs,
      object : BrilliantLaunch.Callback {
        override fun onStarted() = invoke.resolve()
        override fun onFailed(error: Throwable) = invoke.reject(error.message ?: error.toString())
      })
  }

  @Command
  fun keepAlive(invoke: Invoke) {
    val args = invoke.parseArgs(KeepAliveArgs::class.java)
    val service = Intent(activity, DownloadService::class.java)
      .putExtra(DownloadService.EXTRA_TITLE, args.title)
      .putExtra(DownloadService.EXTRA_CHANNEL, args.channel)
    try {
      synchronized(this) {
        if (args.on && !keepingAlive) {
          androidx.core.content.ContextCompat.startForegroundService(activity, service)
          keepingAlive = true
        } else if (!args.on && keepingAlive) {
          keepingAlive = false
          activity.startService(service.setAction(DownloadService.ACTION_STOP))
        }
      }
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject(e.message ?: e.toString())
    }
  }

  @Command
  fun webLogin(invoke: Invoke) {
    val args = invoke.parseArgs(LoginArgs::class.java)
    val intent = Intent(activity, LoginActivity::class.java)
      .putExtra(LoginActivity.EXTRA_URL, args.url)
      .putExtra(LoginActivity.EXTRA_REDIRECT, args.redirect)
      .putExtra(LoginActivity.EXTRA_TITLE, args.title)
      .putExtra(LoginActivity.EXTRA_CLOSE, args.close)
    startActivityForResult(invoke, intent, "webLoginResult")
  }

  @ActivityCallback
  fun webLoginResult(invoke: Invoke, result: ActivityResult) {
    invoke.resolve(JSObject().put("url", result.data?.getStringExtra(LoginActivity.EXTRA_RESULT)))
  }

  @Command
  fun gameRunning(invoke: Invoke) {
    val manager = activity.getSystemService(android.app.ActivityManager::class.java)
    val running = manager.runningAppProcesses?.any { it.processName.endsWith(":game") } == true
    invoke.resolve(JSObject().put("running", running))
  }

  @Command
  fun resumeGame(invoke: Invoke) {
    val manager = activity.getSystemService(android.app.ActivityManager::class.java)
    val task = manager.appTasks.firstOrNull { it.taskInfo.baseActivity?.className == "net.kdt.pojavlaunch.MainActivity" }
    task?.moveToFront()
    invoke.resolve(JSObject().put("found", task != null))
  }

  @Command
  fun stopGame(invoke: Invoke) {
    val manager = activity.getSystemService(android.app.ActivityManager::class.java)
    manager.runningAppProcesses?.filter { it.processName.endsWith(":game") }?.forEach { android.os.Process.killProcess(it.pid) }
    invoke.resolve()
  }

  @Command
  fun openFile(invoke: Invoke) {
    try {
      val file = File(invoke.parseArgs(OpenFileArgs::class.java).path)
      val uri = androidx.core.content.FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
      val mime = android.webkit.MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase()) ?: "text/plain"
      val view = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      activity.startActivity(Intent.createChooser(view, null))
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject(e.message ?: e.toString())
    }
  }

  @Command
  fun stage(invoke: Invoke) {
    val uri = Uri.parse(invoke.parseArgs(StageArgs::class.java).uri)
    Thread {
      try {
        val resolver = activity.contentResolver
        val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
          if (it.moveToFirst()) it.getString(0) else null
        } ?: "picked"
        val dir = File(activity.cacheDir, "picked/${System.nanoTime()}").apply { mkdirs() }
        val out = File(dir, File(name).name)
        resolver.openInputStream(uri)?.use { input -> out.outputStream().use { input.copyTo(it) } }
          ?: throw IllegalStateException("無法開啟選取的檔案")
        invoke.resolve(JSObject().put("path", out.absolutePath))
      } catch (e: Exception) {
        invoke.reject(e.message ?: e.toString())
      }
    }.start()
  }
}
