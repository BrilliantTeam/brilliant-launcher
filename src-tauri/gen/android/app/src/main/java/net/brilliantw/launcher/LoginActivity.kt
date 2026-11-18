package net.brilliantw.launcher

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.view.Gravity
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import android.view.ViewGroup.LayoutParams.WRAP_CONTENT
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class LoginActivity : AppCompatActivity() {
  private lateinit var web: WebView
  private lateinit var redirect: String

  @SuppressLint("SetJavaScriptEnabled")
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    redirect = intent.getStringExtra(EXTRA_REDIRECT).orEmpty()
    val density = resources.displayMetrics.density
    val bar = LinearLayout(this).apply {
      orientation = LinearLayout.HORIZONTAL
      gravity = Gravity.CENTER_VERTICAL
      setBackgroundColor(Color.rgb(0x1b, 0x1d, 0x21))
      setPadding((12 * density).toInt(), 0, (4 * density).toInt(), 0)
      addView(TextView(this@LoginActivity).apply {
        text = intent.getStringExtra(EXTRA_TITLE)
        setTextColor(Color.WHITE)
        textSize = 16f
      }, LinearLayout.LayoutParams(0, WRAP_CONTENT, 1f))
      addView(Button(this@LoginActivity).apply {
        text = intent.getStringExtra(EXTRA_CLOSE)
        minHeight = (48 * density).toInt()
        setOnClickListener { finishWith(null) }
      }, LinearLayout.LayoutParams(WRAP_CONTENT, WRAP_CONTENT))
    }
    web = WebView(this).apply {
      settings.javaScriptEnabled = true
      settings.domStorageEnabled = true
      webViewClient = object : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
          val url = request.url.toString()
          if (!url.startsWith(redirect)) return false
          finishWith(url)
          return true
        }

        override fun onPageStarted(view: WebView, url: String, favicon: android.graphics.Bitmap?) {
          if (url.startsWith(redirect)) finishWith(url)
        }
      }
    }
    setContentView(LinearLayout(this).apply {
      orientation = LinearLayout.VERTICAL
      addView(bar, LinearLayout.LayoutParams(MATCH_PARENT, (56 * density).toInt()))
      addView(web, LinearLayout.LayoutParams(MATCH_PARENT, 0, 1f))
    })
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        if (web.canGoBack()) web.goBack() else finishWith(null)
      }
    })
    if (savedInstanceState == null) web.loadUrl(intent.getStringExtra(EXTRA_URL).orEmpty())
  }

  private fun finishWith(url: String?) {
    if (isFinishing) return
    setResult(if (url == null) RESULT_CANCELED else RESULT_OK, Intent().putExtra(EXTRA_RESULT, url))
    finish()
  }

  override fun onDestroy() {
    web.destroy()
    super.onDestroy()
  }

  companion object {
    const val EXTRA_URL = "url"
    const val EXTRA_REDIRECT = "redirect"
    const val EXTRA_TITLE = "title"
    const val EXTRA_CLOSE = "close"
    const val EXTRA_RESULT = "result"
  }

  override fun onWindowFocusChanged(hasFocus: Boolean) {
    super.onWindowFocusChanged(hasFocus)
    if (hasFocus) WindowCompat.getInsetsController(window, window.decorView).apply {
      systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
      hide(WindowInsetsCompat.Type.systemBars())
    }
  }
}
