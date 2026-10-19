//! macOS: PDFKit's print operation and panel.

use std::path::Path;

/// Shows the print panel for `pdf`; false when cancelled.
pub async fn print_pdf(pdf: &Path) -> anyhow::Result<bool> {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSPaperOrientation, NSPrintInfo, NSPrintJobSavingURL, NSPrintSaveJob};
    use objc2_foundation::{NSCopying, NSSize, NSString, NSURL};
    use objc2_pdf_kit::{PDFDisplayBox, PDFDocument, PDFPrintScalingMode};

    let mtm =
        MainThreadMarker::new().ok_or_else(|| anyhow::anyhow!("printing needs the main thread"))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&pdf.to_string_lossy()));
    let document = unsafe { PDFDocument::initWithURL(PDFDocument::alloc(), &url) }
        .ok_or_else(|| anyhow::anyhow!("{} is no readable PDF", pdf.display()))?;
    let sizes: Vec<(f64, f64)> = (0..unsafe { document.pageCount() })
        .filter_map(|n| unsafe { document.pageAtIndex(n) })
        .map(|page| {
            let size = unsafe { page.boundsForBox(PDFDisplayBox::MediaBox) }.size;
            // A page turned a quarter shows its other side up.
            match unsafe { page.rotation() }.rem_euclid(180) == 90 {
                true => (size.height, size.width),
                false => (size.width, size.height),
            }
        })
        .collect();
    let (width, height) = super::paper_for(&sizes).ok_or_else(|| anyhow::anyhow!("{} has no pages", pdf.display()))?;
    let paper = NSSize::new(width, height);
    if let Some(scale) = crate::env::value("ELY_PRINT_SHARED_SCALE") {
        // Tests start from a shared scale the copy must undo.
        NSPrintInfo::sharedPrintInfo().setScalingFactor(scale.parse()?);
    }
    // The PDF already holds the document's paper, margins and scale.
    let info = NSPrintInfo::sharedPrintInfo().copy();
    info.setScalingFactor(1.0);
    info.setOrientation(match paper.width > paper.height {
        true => NSPaperOrientation::Landscape,
        false => NSPaperOrientation::Portrait,
    });
    info.setPaperSize(paper);
    info.setTopMargin(0.0);
    info.setBottomMargin(0.0);
    info.setLeftMargin(0.0);
    info.setRightMargin(0.0);
    log::info!("print: paper {}x{} points", paper.width, paper.height);
    let operation = unsafe {
        document.printOperationForPrintInfo_scalingMode_autoRotate(
            Some(&info),
            PDFPrintScalingMode::PageScaleNone,
            true,
            mtm,
        )
    }
    .ok_or_else(|| anyhow::anyhow!("no print operation for {}", pdf.display()))?;
    // Tests read the real job: saved to a file.
    let saved = crate::env::value("ELY_PRINT_TO");
    if let Some(to) = &saved {
        let info = operation.printInfo();
        info.setJobDisposition(unsafe { NSPrintSaveJob });
        let to = NSURL::fileURLWithPath(&NSString::from_str(to));
        unsafe { info.dictionary().insert(NSPrintJobSavingURL, &to) };
    }
    operation.setShowsPrintPanel(saved.is_none());
    operation.setShowsProgressPanel(saved.is_none());
    let printed = operation.runOperation();
    log::info!("print: the panel closed, printed {printed}");
    Ok(printed)
}
